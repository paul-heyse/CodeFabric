//! Source authorization is read from the live workspace policy, independently of fact visibility.

use crate::operational_store::OperationalReaderFactory;
use crate::workspace_registry::SourceDisclosureGrant;

#[derive(Clone, Eq, Hash, PartialEq)]
pub(crate) struct SourceDisclosureAuthority {
    reader: OperationalReaderFactory,
    workspace: [u8; 16],
}

impl std::fmt::Debug for SourceDisclosureAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceDisclosureAuthority")
            .field("workspace", &self.workspace)
            .finish_non_exhaustive()
    }
}

impl SourceDisclosureAuthority {
    pub(crate) const fn new(reader: OperationalReaderFactory, workspace: [u8; 16]) -> Self {
        Self { reader, workspace }
    }

    pub(crate) fn authorize(&self) -> Result<SourceDisclosureGrant, String> {
        let reader = self.reader.open().map_err(|error| error.to_string())?;
        crate::workspace_registry::read_source_disclosure(&reader, self.workspace)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "source disclosure is not authorized for this workspace".to_owned())
    }
}
