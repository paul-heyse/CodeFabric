//! Application-owned observations of actual wrapper inputs. These do not authorize replay.

use super::{CompilationBegin, Status, valid_digest};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcInvocationObservation {
    pub compiler_path: Vec<u8>,
    pub working_directory: Vec<u8>,
    pub source_path: Vec<u8>,
    pub source_content_digest: String,
    pub arguments: Vec<Vec<u8>>,
    pub environment: Vec<(Vec<u8>, String)>,
    pub normalized_invocation_digest: String,
    pub package_id: String,
    pub target_name: String,
    pub target_kind: String,
    pub crate_name: String,
    pub crate_type: String,
}

pub(super) fn validate(begin: &CompilationBegin) -> Result<(), Status> {
    let Some(census) = &begin.invocation_census else {
        return Ok(());
    };
    let valid_path = |bytes: &[u8]| !bytes.is_empty() && !bytes.contains(&0);
    if !valid_path(&census.compiler_path)
        || !valid_path(&census.working_directory)
        || !valid_path(&census.source_path)
        || !valid_digest(&census.source_content_digest)
        || census.arguments.is_empty()
        || census
            .arguments
            .iter()
            .any(|argument| argument.contains(&0))
        || census.environment.iter().any(|entry| {
            !valid_path(&entry.name)
                || entry.name.contains(&b'=')
                || !valid_digest(&entry.value_digest)
        })
        || census
            .environment
            .windows(2)
            .any(|pair| pair[0].name >= pair[1].name)
    {
        return Err(Status::invalid_argument(
            "invalid compiler invocation census",
        ));
    }
    Ok(())
}

pub(super) fn take(begin: &mut CompilationBegin) -> Option<RustcInvocationObservation> {
    let census = begin.invocation_census.take()?;
    let target = begin.target.take().expect("validated compilation target");
    Some(RustcInvocationObservation {
        compiler_path: census.compiler_path,
        working_directory: census.working_directory,
        source_path: census.source_path,
        source_content_digest: census.source_content_digest,
        arguments: census.arguments,
        environment: census
            .environment
            .into_iter()
            .map(|entry| (entry.name, entry.value_digest))
            .collect(),
        normalized_invocation_digest: std::mem::take(&mut begin.normalized_rustc_invocation_digest),
        package_id: target.package_id,
        target_name: target.target_name,
        target_kind: target.target_kind,
        crate_name: target.crate_name,
        crate_type: target.crate_type,
    })
}

impl RustcInvocationObservation {
    pub(super) fn retained_bytes(&self) -> Result<u64, Status> {
        let capacities = [
            self.compiler_path.capacity(),
            self.working_directory.capacity(),
            self.source_path.capacity(),
            self.source_content_digest.capacity(),
            self.normalized_invocation_digest.capacity(),
            self.package_id.capacity(),
            self.target_name.capacity(),
            self.target_kind.capacity(),
            self.crate_name.capacity(),
            self.crate_type.capacity(),
        ];
        let sizes = [
            std::mem::size_of::<Self>(),
            self.arguments
                .capacity()
                .saturating_mul(std::mem::size_of::<Vec<u8>>()),
            self.environment
                .capacity()
                .saturating_mul(std::mem::size_of::<(Vec<u8>, String)>()),
        ];
        sizes
            .into_iter()
            .chain(capacities)
            .chain(self.arguments.iter().map(Vec::capacity))
            .chain(
                self.environment
                    .iter()
                    .flat_map(|(name, digest)| [name.capacity(), digest.capacity()]),
            )
            .try_fold(0_u64, |sum, bytes| sum.checked_add(bytes as u64))
            .ok_or_else(|| Status::resource_exhausted("compiler census size overflow"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::generated::codefabric::rustc::v1::{
        PackageTargetIdentity, RustcEnvironmentObservation, RustcInvocationCensus,
    };
    use prost::Message;

    #[test]
    fn census_preserves_native_bytes_and_absence_and_rejects_ambiguous_environment() {
        let digest = crate::integrity::frame_digest(crate::integrity::digest_bytes(b"observed"));
        let mut begin = CompilationBegin::default();
        validate(&begin).unwrap();
        assert!(take(&mut begin).is_none());
        begin.target = Some(PackageTargetIdentity::default());
        begin.invocation_census = Some(RustcInvocationCensus {
            compiler_path: b"/dependencies/rustc".to_vec(),
            working_directory: b"/workspace/raw-\xff".to_vec(),
            source_path: b"src/lib.rs".to_vec(),
            source_content_digest: digest.clone(),
            arguments: vec![b"--cfg".to_vec(), b"value=\"raw-\xff\"".to_vec()],
            environment: vec![RustcEnvironmentObservation {
                name: b"CARGO_PKG_NAME".to_vec(),
                value_digest: digest,
            }],
        });
        let mut decoded = CompilationBegin::decode(begin.encode_to_vec().as_slice()).unwrap();
        validate(&decoded).unwrap();
        let observed = take(&mut decoded).unwrap();
        assert_eq!(observed.arguments[1], b"value=\"raw-\xff\"");
        assert_eq!(observed.working_directory, b"/workspace/raw-\xff");
        assert!(observed.retained_bytes().unwrap() > std::mem::size_of_val(&observed) as u64);
        let census = begin.invocation_census.as_mut().unwrap();
        census.environment.push(census.environment[0].clone());
        assert_eq!(
            validate(&begin).unwrap_err().code(),
            tonic::Code::InvalidArgument
        );
        begin.invocation_census.as_mut().unwrap().environment.pop();
        begin
            .invocation_census
            .as_mut()
            .unwrap()
            .arguments
            .push(b"embedded\0nul".to_vec());
        assert!(validate(&begin).is_err());
    }
}
