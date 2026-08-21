# Arrow/Delta contract outputs

`table-specs.json` is the canonical review projection of
`contracts/schema/schema-contract-ir.json`. The same cataloged compilation unit emits
`src/generated/table_specs.rs` and `contracts/schema/operational-store.sql`; none is an
independent authority and all are byte-for-byte reproducible.
