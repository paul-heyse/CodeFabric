# Native dependency sources

These are ordinary editable sources tracked by the enclosing CodeFabric Git repository.
All native integration work uses these directories. Previous temporary stages are frozen.
No source packaging or patch replay is required before editing, building, or integrating.

The import preserves current in-progress sources, including unvalidated changes.
A Git checkpoint is not a passing build or WP79 completion. Upstream licenses and notices
remain with the source. Cargo patches select this one copy of each package.
Generated target outputs and nested Git metadata were excluded from the import.

| Source | Upstream origin | Previous staging checkout | Staging HEAD at import |
|---|---|---|---|
| `buoyant_kernel` | `0.25.1` | `/tmp/codefabric-kernel-resource-nkpsdpu3/buoyant_kernel-0.25.1` | `406559d3422eb2b00b17203637c57523c628cae9` |
| `buoyant_kernel_engine` | `0.25.0` | `/tmp/codefabric-kernel-resource-nkpsdpu3/buoyant_kernel_engine-0.25.0` | `f0feacffc33b315170f9b7d09cfdc4a2f7522f1c` |
| `buoyant_kernel_derive` | `1.1.0` | `/tmp/codefabric-kernel-resource-nkpsdpu3/buoyant_kernel_derive-1.1.0` | `cd03b1ed6957fb7850ede5e33340c9c8d03b60bb` |
| `arrow-json` | `59.2.0` | `/tmp/codefabric-kernel-resource-nkpsdpu3/arrow-json-59.2.0` | `568496128142567f95d79500d918e2e5b4c7952e` |
| `parquet` | `59.2.0` | `/tmp/codefabric-parquet-resource-OTA4BLiK/parquet` | `8b6ed83af78501b195fc5a9f14bbdaeced0976a6` |
| `arrow-buffer` | `59.2.0` | `/tmp/codefabric-parquet-resource-OTA4BLiK/arrow-buffer` | `d2a041a46fbb37b804f24d8e458895167cf2fc13` |
| `arrow-schema` | `59.2.0` | `/tmp/codefabric-parquet-resource-OTA4BLiK/arrow-schema` | `d0084c248b528763f4492f7b425ea089001359fc` |
| `arrow-data` | `59.2.0` | `/tmp/codefabric-parquet-resource-OTA4BLiK/arrow-data` | `62df5f9c6deffb983d6117b97a3bb2f27dc6d56c` |
| `arrow-array` | `59.2.0` | `/tmp/codefabric-native-lane-e6p7jmz_/arrow-array-59.2.0` | `8c0abd707ca702881837293a5b22510d18c8ac3b` |
| `arrow-select` | `59.2.0` | `/tmp/codefabric-parquet-resource-OTA4BLiK/arrow-select` | `1575a4ff9844edef47920d05ca39140a75dce3a5` |
| `delta-rs` | `43a0cf10a313e5077c48637ad786a05359136bbb` | `/tmp/codefabric-delta-retained-muuzjq_2/delta-rs` | `43a0cf10a313e5077c48637ad786a05359136bbb` |
| `tokio` | `1.53.1` | `/tmp/codefabric-tokio-resource-dhe9di5h/tokio` | `4f4d69121edc72b80bc98af6e1757cd1078dcc13` |
