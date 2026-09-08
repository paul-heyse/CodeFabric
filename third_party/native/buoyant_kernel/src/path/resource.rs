//! Original native file and path-container ownership. Compatibility values are
//! deliberately unadmitted; governed constructors reserve before allocation.
use super::ParsedLogPath;
use crate::crc::resource::{add, mul};
use crate::resource::{AllocationRequest, NativeResourceScope};
use crate::{DeltaResult, FileMeta};
use std::ops::Deref;
use std::sync::Arc;
use url::Url;

pub(crate) fn scope() -> Option<Arc<NativeResourceScope>> {
    crate::resource::current_resource_scope()
}
pub(crate) fn reserve(
    scope: &Option<Arc<NativeResourceScope>>,
    kind: &'static str,
    bytes: usize,
) -> DeltaResult<()> {
    if let Some(scope) = scope {
        scope.reserve(AllocationRequest { kind, bytes })?;
    }
    Ok(())
}
pub(crate) fn validate(owner: Option<&Arc<NativeResourceScope>>) -> DeltaResult<()> {
    if let Some(current) = scope() {
        match owner {
            Some(_) => current.check_available(),
            _ => Err(crate::resource::ResourceExhausted {
                kind: "native_log_unadmitted_owner",
                requested: 1,
                limit: 0,
            }
            .into()),
        }
    } else {
        Ok(())
    }
}

/// Move-only original filesystem-list result. Borrowing cannot detach its URL
/// from the preallocation receipt. Parsing transfers both into the parsed owner.
#[derive(Debug)]
pub struct OwnedFileMeta {
    value: FileMeta,
    scope: Option<Arc<NativeResourceScope>>,
}
impl Deref for OwnedFileMeta {
    type Target = FileMeta;
    fn deref(&self) -> &FileMeta {
        &self.value
    }
}
impl PartialEq for OwnedFileMeta {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for OwnedFileMeta {}
impl PartialOrd for OwnedFileMeta {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for OwnedFileMeta {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}
impl OwnedFileMeta {
    pub(crate) fn from_admitted_parts(value: FileMeta, scope: Option<Arc<NativeResourceScope>>) -> Self { Self { value, scope } }
    /// Compatibility ingress carries no resource proof. A governed parser rejects it.
    pub fn unadmitted(value: FileMeta) -> Self {
        Self { value, scope: None }
    }
    pub fn resource_scope(&self) -> Option<&Arc<NativeResourceScope>> {
        self.scope.as_ref()
    }
    /// Copy a borrowed native URL only after admitting its exact String clone.
    pub fn try_copy_url(location: &Url, last_modified: i64, size: u64) -> DeltaResult<Self> {
        let scope = scope();
        reserve(
            &scope,
            "native_file_meta_url_copy",
            add(location.as_str().len(), std::mem::size_of::<Self>())?,
        )?;
        Ok(Self {
            value: FileMeta {
                location: location.clone(),
                last_modified,
                size,
            },
            scope,
        })
    }
    /// Exact native listing transformation: clone the borrowed base URL, then
    /// apply the original object path through url 2.5.8's native set_path.
    pub fn try_from_object_path(
        base: &Url,
        object_path: &str,
        last_modified: i64,
        size: u64,
    ) -> DeltaResult<Self> {
        Self::try_from_object_path_admitted(base, object_path, last_modified, size, scope())
    }
    /// Explicit captured native policy, usable from the listing future's worker.
    pub fn try_from_object_path_admitted(
        base: &Url,
        object_path: &str,
        last_modified: i64,
        size: u64,
        scope: Option<Arc<NativeResourceScope>>,
    ) -> DeltaResult<Self> {
        let scope = crate::crc::resource::select_scope(scope, None)?;
        // Url::clone copies serialization at len. set_path saves the suffix,
        // percent-encodes each input byte into at most three bytes, and String
        // grows by max(2*capacity, required, 8). Include old/new coexistence,
        // the slash-prefixed formatting String, and suffix copy.
        let input = add(object_path.len(), 1)?;
        let expanded = add(base.as_str().len(), mul(input, 3)?)?;
        let strings = add(
            mul(add(expanded, 8)?, 4)?,
            add(mul(add(input, 8)?, 2)?, base.as_str().len())?,
        )?;
        reserve(
            &scope,
            "native_listing_url",
            add(strings, std::mem::size_of::<Self>())?,
        )?;
        let mut location = base.clone();
        location.set_path(&format!("/{object_path}"));
        Ok(Self {
            value: FileMeta {
                location,
                last_modified,
                size,
            },
            scope,
        })
    }
    pub fn into_parsed(self) -> DeltaResult<Option<ParsedLogPath>> {
        validate(self.scope.as_ref())?;
        ParsedLogPath::try_from_owned(self.value, self.scope)
    }
}

#[derive(Debug)]
struct LogPathsInner {
    values: Vec<ParsedLogPath>,
    scope: Option<Arc<NativeResourceScope>>,
}
/// Immutable shared original Vec backing. Mutation and new descriptor storage
/// use fallible admission; iteration retains the original Vec until it ends.
#[derive(Debug, Clone, Default)]
pub struct OwnedLogPaths(Option<Arc<LogPathsInner>>);
impl Deref for OwnedLogPaths {
    type Target = [ParsedLogPath];
    fn deref(&self) -> &[ParsedLogPath] {
        self.as_slice()
    }
}
impl PartialEq for OwnedLogPaths {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for OwnedLogPaths {}
impl PartialEq<Vec<ParsedLogPath>> for OwnedLogPaths {
    fn eq(&self, other: &Vec<ParsedLogPath>) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl OwnedLogPaths {
    pub fn as_slice(&self) -> &[ParsedLogPath] {
        self.0.as_ref().map_or(&[], |inner| inner.values.as_slice())
    }
    pub fn capacity(&self) -> usize {
        self.0.as_ref().map_or(0, |inner| inner.values.capacity())
    }
    pub fn resource_scope(&self) -> Option<&Arc<NativeResourceScope>> {
        self.0.as_ref().and_then(|inner| inner.scope.as_ref())
    }
    pub(crate) fn validate(&self) -> DeltaResult<()> {
        if self.is_empty() {
            Ok(())
        } else {
            validate(self.resource_scope())
        }
    }
    pub fn try_from_iter(
        iter: impl IntoIterator<Item = DeltaResult<ParsedLogPath>>,
    ) -> DeltaResult<Self> {
        let mut result = Self::default();
        for item in iter {
            result.try_push(item?)?;
        }
        Ok(result)
    }
    fn make_mut(&mut self, additional: usize) -> DeltaResult<&mut Vec<ParsedLogPath>> {
        self.validate()?;
        let desired = add(self.len(), additional)?;
        let scope = scope().or_else(|| self.resource_scope().cloned());
        let shared = self.0.as_ref().is_some_and(|inner| {
            Arc::strong_count(inner) != 1
                || scope.as_ref().is_some_and(|current| {
                    inner
                        .scope
                        .as_ref()
                        .is_none_or(|old| !Arc::ptr_eq(old, current))
                })
        });
        if self.0.is_none() || shared {
            let capacity = desired.max(4);
            let bytes = add(
                mul(capacity, std::mem::size_of::<ParsedLogPath>())?,
                add(
                    std::mem::size_of::<LogPathsInner>(),
                    2 * std::mem::size_of::<usize>(),
                )?,
            )?;
            reserve(&scope, "native_log_path_descriptors", bytes)?;
            let mut values = Vec::with_capacity(capacity);
            values.extend(self.iter().cloned()); // ParsedLogPath clones share original backing.
            self.0 = Some(Arc::new(LogPathsInner { values, scope }));
        }
        let inner = Arc::get_mut(self.0.as_mut().unwrap()).unwrap();
        if desired > inner.values.capacity() {
            let capacity = desired.max(mul(inner.values.capacity(), 2)?);
            reserve(
                &inner.scope,
                "native_log_path_descriptors",
                mul(capacity, std::mem::size_of::<ParsedLogPath>())?,
            )?;
            inner.values.reserve_exact(capacity - inner.values.len());
        }
        Ok(&mut inner.values)
    }
    pub fn try_push(&mut self, value: ParsedLogPath) -> DeltaResult<()> {
        value.validate_owner()?;
        self.make_mut(1)?.push(value);
        Ok(())
    }
    pub(crate) fn try_retain(
        &mut self,
        keep: impl FnMut(&ParsedLogPath) -> bool,
    ) -> DeltaResult<()> {
        if !self.is_empty() {
            self.make_mut(0)?.retain(keep);
        }
        Ok(())
    }
    pub(crate) fn try_drain_prefix(&mut self, count: usize) -> DeltaResult<()> {
        if count != 0 {
            self.make_mut(0)?.drain(..count);
        }
        Ok(())
    }
    pub(crate) fn clear(&mut self) {
        self.0 = None;
    }
    pub(crate) fn try_extend(
        &mut self,
        values: impl IntoIterator<Item = ParsedLogPath>,
    ) -> DeltaResult<()> {
        for value in values {
            self.try_push(value)?;
        }
        Ok(())
    }
}
impl From<Vec<ParsedLogPath>> for OwnedLogPaths {
    fn from(values: Vec<ParsedLogPath>) -> Self {
        if values.is_empty() {
            Self::default()
        } else {
            Self(Some(Arc::new(LogPathsInner {
                values,
                scope: None,
            })))
        }
    }
}
/// Holds the actual original descriptor backing even if the outer list is dropped.
pub struct OwnedLogPathIterator {
    paths: OwnedLogPaths,
    front: usize,
    back: usize,
}
impl Iterator for OwnedLogPathIterator {
    type Item = ParsedLogPath;
    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let value = self.paths[self.front].clone();
        self.front += 1;
        Some(value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.back - self.front;
        (n, Some(n))
    }
}
impl DoubleEndedIterator for OwnedLogPathIterator {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        Some(self.paths[self.back].clone())
    }
}
impl ExactSizeIterator for OwnedLogPathIterator {}
impl IntoIterator for OwnedLogPaths {
    type Item = ParsedLogPath;
    type IntoIter = OwnedLogPathIterator;
    fn into_iter(self) -> Self::IntoIter {
        let back = self.len();
        OwnedLogPathIterator {
            paths: self,
            front: 0,
            back,
        }
    }
}
impl<'a> IntoIterator for &'a OwnedLogPaths {
    type Item = &'a ParsedLogPath;
    type IntoIter = std::slice::Iter<'a, ParsedLogPath>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Source-bounded native relative join for kernel-generated ASCII log names.
/// url 2.5.8 initializes serialization at input length, copies the base URL's
/// scheme/authority/path, then appends the relative path with geometric String
/// growth. These names cannot enter absolute-URL, IDNA, query, or fragment parse.
pub(crate) fn try_join_url(base: &Url, relative: &str) -> DeltaResult<Url> {
    let scope = scope();
    if scope.is_some()
        && !relative
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_./-".contains(&byte))
    {
        return Err(crate::resource::ResourceExhausted {
            kind: "native_generated_log_name",
            requested: relative.len(),
            limit: 0,
        }
        .into());
    }
    let serialization = add(add(base.as_str().len(), relative.len())?, 8)?;
    // Old and replacement serialization may coexist; relative parsing copies a
    // base prefix and uses at most twice final capacity (minimum eight bytes).
    reserve(&scope, "native_log_path_join", mul(serialization, 4)?)?;
    Ok(base.join(relative)?)
}
pub(crate) fn admit_file_slice_descriptors(count: usize) -> DeltaResult<()> {
    reserve(
        &scope(),
        "native_log_read_descriptors",
        mul(count, std::mem::size_of::<crate::FileSlice>())?,
    )
}

#[derive(Debug)]
struct LogUrlInner {
    url: Url,
    _scope: Option<Arc<NativeResourceScope>>,
}
/// Original immutable URL copy and the admission that preceded its construction.
#[derive(Debug, Clone)]
pub struct OwnedLogUrl(Arc<LogUrlInner>);
impl Deref for OwnedLogUrl { type Target = Url; fn deref(&self) -> &Url { &self.0.url } }
impl AsRef<Url> for OwnedLogUrl { fn as_ref(&self) -> &Url { &self.0.url } }
impl std::fmt::Display for OwnedLogUrl { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.0.url.fmt(f) } }
impl PartialEq for OwnedLogUrl { fn eq(&self, other: &Self) -> bool { self.0.url == other.0.url } }
impl Eq for OwnedLogUrl {}
impl PartialEq<Url> for OwnedLogUrl { fn eq(&self, other: &Url) -> bool { &self.0.url == other } }
impl OwnedLogUrl {
    pub fn try_copy(url: &Url) -> DeltaResult<Self> {
        let scope = scope();
        reserve(&scope, "native_log_root_owner", add(url.as_str().len(), add(std::mem::size_of::<LogUrlInner>(), 2 * std::mem::size_of::<usize>())?)?)?;
        Ok(Self(Arc::new(LogUrlInner { url: url.clone(), _scope: scope })))
    }
    pub fn resource_scope(&self) -> Option<&Arc<NativeResourceScope>> { self.0._scope.as_ref() }
    pub(crate) fn try_clone_url_admitted(&self) -> DeltaResult<Url> {
        let scope = scope().or_else(|| self.resource_scope().cloned());
        reserve(&scope, "native_log_url_copy", self.as_str().len())?;
        Ok(self.0.url.clone())
    }
}

/// Original read-selection descriptors and copied native URLs. A slice borrows
/// the actual backing; the owning iterator retains the descriptor allocation.
#[derive(Debug, Default)]
pub struct OwnedFileMetas {
    values: Vec<FileMeta>,
    scope: Option<Arc<NativeResourceScope>>,
}
impl Deref for OwnedFileMetas { type Target = [FileMeta]; fn deref(&self) -> &[FileMeta] { &self.values } }
impl OwnedFileMetas {
    pub fn try_copy(files: &[FileMeta]) -> DeltaResult<Self> { Self::try_copy_admitted(files, scope()) }
    pub fn try_copy_admitted(files: &[FileMeta], explicit: Option<Arc<NativeResourceScope>>) -> DeltaResult<Self> {
        let scope = crate::crc::resource::select_scope(explicit, None)?;
        let mut bytes = mul(files.len(), std::mem::size_of::<FileMeta>())?;
        for file in files { bytes = add(bytes, file.location.as_str().len())?; }
        reserve(&scope, "native_read_file_descriptors", bytes)?;
        Ok(Self { values: files.to_vec(), scope })
    }
    pub(crate) fn try_push_copy(&mut self, file: &FileMeta) -> DeltaResult<()> {
        let current = scope();
        if self.scope.is_none() { self.scope = current; }
        else if current.as_ref().is_some_and(|current| !Arc::ptr_eq(current, self.scope.as_ref().unwrap())) {
            return Err(crate::resource::ResourceExhausted { kind: "native_read_descriptor_scope", requested: 1, limit: 0 }.into());
        }
        if self.values.len() == self.values.capacity() {
            let capacity = mul(self.values.capacity(), 2)?.max(4);
            reserve(&self.scope, "native_read_file_descriptors", mul(capacity, std::mem::size_of::<FileMeta>())?)?;
            self.values.reserve_exact(capacity - self.values.len());
        }
        reserve(&self.scope, "native_read_file_url", file.location.as_str().len())?;
        self.values.push(file.clone());
        Ok(())
    }
    pub(crate) fn try_push_owned(&mut self, file: OwnedFileMeta) -> DeltaResult<()> {
        // Selected references are created in the same current operation. Mixing
        // owners requires an independently retained owner vector, so copy them
        // through the admitted ingress instead of dropping their original owner.
        let current = scope();
        if file.scope.as_ref().zip(current.as_ref()).is_some_and(|(owner, current)| !Arc::ptr_eq(owner, current)) {
            return self.try_push_copy(&file.value);
        }
        if self.scope.is_none() { self.scope = current.or_else(|| file.scope.clone()); }
        if self.values.len() == self.values.capacity() {
            let capacity = mul(self.values.capacity(), 2)?.max(4);
            reserve(&self.scope, "native_read_file_descriptors", mul(capacity, std::mem::size_of::<FileMeta>())?)?;
            self.values.reserve_exact(capacity - self.values.len());
        }
        self.values.push(file.value);
        Ok(())
    }
    pub(crate) fn pop(&mut self) { self.values.pop(); }
    pub(crate) fn reverse(&mut self) { self.values.reverse(); }
    pub fn resource_scope(&self) -> Option<&Arc<NativeResourceScope>> { self.scope.as_ref() }
}
impl AsRef<[FileMeta]> for OwnedFileMetas { fn as_ref(&self) -> &[FileMeta] { &self.values } }
pub struct OwnedFileMetaIterator {
    values: std::vec::IntoIter<FileMeta>,
    scope: Option<Arc<NativeResourceScope>>,
}
impl Iterator for OwnedFileMetaIterator {
    type Item = OwnedFileMeta;
    fn next(&mut self) -> Option<Self::Item> {
        self.values.next().map(|value| OwnedFileMeta { value, scope: self.scope.clone() })
    }
    fn size_hint(&self) -> (usize, Option<usize>) { self.values.size_hint() }
}
impl ExactSizeIterator for OwnedFileMetaIterator {}
impl IntoIterator for OwnedFileMetas {
    type Item = OwnedFileMeta;
    type IntoIter = OwnedFileMetaIterator;
    fn into_iter(self) -> Self::IntoIter { OwnedFileMetaIterator { values: self.values.into_iter(), scope: self.scope } }
}
