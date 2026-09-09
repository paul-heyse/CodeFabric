# Native dependency selection

The allocation-receipt forks were retired on 2026-09-09. The root manifest and
lock now select upstream Arrow/Parquet 59.2.0, Tokio 1.53.1, Buoyant kernel
0.25.1, engine 0.25.0 and derive 1.1.0, plus delta-rs revision
`43a0cf10a313e5077c48637ad786a05359136bbb`. No dependency was downgraded.

The removed changes supplied allocation admission, retained-allocation receipts,
policy propagation and wrapper types. Their production consumer is replaced by
bounded owned runtime lanes, shared DataFusion pools, bounded store operations,
provider containment and application result/state limits. Upstream code retains
native transaction, multipart cleanup, task cancellation and maintenance behavior;
application cancellation and runtime joining remain required.

Git retains the old forks if a concrete regression requires a small correctness
fix. Add a local patch only for such a consumer, with the relevant behavior test.
`just stable-graph-check` checks the current source and version identities.
