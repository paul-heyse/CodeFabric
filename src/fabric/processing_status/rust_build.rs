//! Optional typed build selections remain readable on older processing snapshots.

use super::{Array, RecordBatch, StringArray, strings};
use arrow_array::{BooleanArray, ListArray};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProcessingRustBuildSelection {
    pub profile: String,
    pub features: Vec<String>,
    pub default_features: bool,
}

impl ProcessingRustBuildSelection {
    pub(super) fn valid(&self) -> bool {
        !self.profile.is_empty()
            && self.profile.len() <= 16_384
            && self.features.len() <= 1024
            && self
                .features
                .iter()
                .all(|value| !value.is_empty() && value.len() <= 16_384)
            && self.features.iter().map(String::len).sum::<usize>() <= 65_536
    }
}

pub(super) struct RustBuildColumns<'a> {
    profiles: &'a StringArray,
    features: &'a ListArray,
    defaults: &'a BooleanArray,
}

impl<'a> RustBuildColumns<'a> {
    pub(super) fn read(batch: &'a RecordBatch) -> Result<Option<Self>, String> {
        let present = ["build_profile", "build_features", "default_features"]
            .iter()
            .filter(|name| batch.column_by_name(name).is_some())
            .count();
        if present == 0 {
            return Ok(None);
        }
        if present != 3 {
            return Err("incomplete processing build-selection schema".into());
        }
        let columns = Self {
            profiles: strings(batch, "build_profile")?,
            features: batch
                .column_by_name("build_features")
                .and_then(|array| array.as_any().downcast_ref())
                .ok_or("invalid processing build features")?,
            defaults: batch
                .column_by_name("default_features")
                .and_then(|array| array.as_any().downcast_ref())
                .ok_or("invalid processing default-feature selection")?,
        };
        if columns
            .features
            .values()
            .as_any()
            .downcast_ref::<StringArray>()
            .is_none()
        {
            return Err("invalid processing build-feature values".into());
        }
        Ok(Some(columns))
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        let values = self
            .features
            .values()
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("checked string list");
        for row in 0..self.profiles.len() {
            if self.profiles.is_null(row) != self.features.is_null(row)
                || self.profiles.is_null(row) != self.defaults.is_null(row)
            {
                return Err("inconsistent processing build-selection presence".into());
            }
            if self.profiles.is_null(row) {
                continue;
            }
            let profile = self.profiles.value(row);
            let offsets = self.features.value_offsets();
            let start = usize::try_from(offsets[row]).expect("valid Arrow list offset");
            let end = usize::try_from(offsets[row + 1]).expect("valid Arrow list offset");
            if profile.is_empty() || profile.len() > 16_384 || end - start > 1024 {
                return Err("invalid processing build selection bounds".into());
            }
            let mut bytes = 0;
            for value in start..end {
                if values.is_null(value)
                    || values.value(value).is_empty()
                    || values.value(value).len() > 16_384
                {
                    return Err("invalid processing build-feature value".into());
                }
                bytes += values.value(value).len();
            }
            if bytes > 65_536 {
                return Err("processing build-feature bytes exceeded".into());
            }
        }
        Ok(())
    }

    pub(super) fn at(&self, row: usize) -> Option<ProcessingRustBuildSelection> {
        if self.profiles.is_null(row) {
            return None;
        }
        let values = self.features.value(row);
        let values = values
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("validated build features");
        Some(ProcessingRustBuildSelection {
            profile: self.profiles.value(row).to_owned(),
            features: values
                .iter()
                .map(|value| value.expect("validated feature").to_owned())
                .collect(),
            default_features: self.defaults.value(row),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{ArrayRef, BinaryArray};
    use std::sync::Arc;

    fn fixture() -> RecordBatch {
        let mut lists =
            arrow_array::builder::ListBuilder::new(arrow_array::builder::StringBuilder::new());
        for features in [None, Some(vec!["b"]), Some(vec![]), Some(vec!["a"])] {
            if let Some(features) = features {
                for feature in features {
                    lists.values().append_value(feature);
                }
                lists.append(true);
            } else {
                lists.append(false);
            }
        }
        let mut columns: Vec<(&str, ArrayRef)> = [
            "family",
            "language",
            "scope_kind",
            "target_name",
            "context_id",
            "processing_state",
            "reason",
        ]
        .into_iter()
        .map(|name| {
            (
                name,
                Arc::new(StringArray::from(vec!["same"; 4])) as ArrayRef,
            )
        })
        .collect();
        columns.extend([
            (
                "relative_path",
                Arc::new(BinaryArray::from_vec(vec![b"Cargo.toml"; 4])) as ArrayRef,
            ),
            (
                "build_profile",
                Arc::new(StringArray::from(vec![
                    None,
                    Some("dev"),
                    Some("dev"),
                    Some("dev"),
                ])),
            ),
            ("build_features", Arc::new(lists.finish())),
            (
                "default_features",
                Arc::new(BooleanArray::from(vec![
                    None,
                    Some(true),
                    Some(false),
                    Some(false),
                ])),
            ),
        ]);
        RecordBatch::try_from_iter(columns).unwrap()
    }

    #[tokio::test]
    async fn processing_rust_build_selection_preserves_absence_empty_features_and_native_order() {
        let batch = fixture();
        let columns = RustBuildColumns::read(&batch).unwrap().unwrap();
        columns.validate().unwrap();
        assert!(columns.at(0).is_none());
        let empty = columns.at(2).unwrap();
        assert!(empty.features.is_empty());
        assert!(!empty.default_features);
        let old = batch.project(&(0..8).collect::<Vec<_>>()).unwrap();
        assert!(RustBuildColumns::read(&old).unwrap().is_none());
        let context = datafusion::prelude::SessionContext::new();
        let sorted =
            super::super::continuation::ordered(context.read_batch(batch.clone()).unwrap())
                .unwrap()
                .collect()
                .await
                .unwrap();
        let selected = sorted
            .iter()
            .flat_map(|batch| {
                let columns = RustBuildColumns::read(batch).unwrap().unwrap();
                (0..batch.num_rows()).map(move |row| columns.at(row))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            selected
                .iter()
                .map(|build| build.as_ref().map(|build| build.features.clone()))
                .collect::<Vec<_>>(),
            [
                None,
                Some(vec![]),
                Some(vec!["a".to_owned()]),
                Some(vec!["b".to_owned()])
            ]
        );
        let mut invalid = batch.columns().to_vec();
        invalid[batch.schema().index_of("default_features").unwrap()] =
            Arc::new(BooleanArray::from(vec![false; 4]));
        let invalid = RecordBatch::try_new(batch.schema(), invalid).unwrap();
        assert!(
            RustBuildColumns::read(&invalid)
                .unwrap()
                .unwrap()
                .validate()
                .is_err()
        );
    }
}
