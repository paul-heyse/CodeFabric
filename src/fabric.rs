//! Delta implementation boundary.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ApplicationTransaction {
    pub(crate) version: i64,
}

pub(crate) fn application_transaction() -> ApplicationTransaction {
    let transaction = deltalake::kernel::Transaction::new("codefabric/compatibility".to_owned(), 1);
    ApplicationTransaction {
        version: transaction.version,
    }
}
