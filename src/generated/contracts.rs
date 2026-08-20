// @generated from contracts/generated/artifact-index.json b3:16a49e4655f903c5f81950bb4c5caf2d2463bc39274b0d23957fb8f411279206; do not edit.
/// One source artifact and its canonical BLAKE3 digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedContractArtifact {
    /// Repository-relative authority path.
    pub path: &'static str,
    /// BLAKE3-256 over the artifact's canonical bytes.
    pub canonical_digest: &'static str,
}

/// BLAKE3 digest of the generated Wave 1 artifact index.
pub const CONTRACT_ARTIFACT_INDEX_DIGEST: &str = "b3:16a49e4655f903c5f81950bb4c5caf2d2463bc39274b0d23957fb8f411279206";

/// Exact AC-G-05 source-artifact index.
pub const CONTRACT_ARTIFACTS: &[GeneratedContractArtifact] = &[
    GeneratedContractArtifact { path: "manifests/suite-manifest.json", canonical_digest: "b3:9d41587a1601e46a322cd19d2a5de80fbf378aa0539b7416859b2bc41a6e2882" },
    GeneratedContractArtifact { path: "manifests/deployment-profile.schema.json", canonical_digest: "b3:c0b0ba9c87a7bb6872605ff81dd0853e8281308cd855a75bfb95a838eaf6a240" },
    GeneratedContractArtifact { path: "manifests/requirements.jsonl", canonical_digest: "b3:44857eb8b770b25fefbe3adb1bd3080a4c591973329e430ead66c8dc8728363d" },
    GeneratedContractArtifact { path: "manifests/traceability.jsonl", canonical_digest: "b3:5c122551f6ceb0694d2df59d56e3c78d49b462fdce9021505cdbcd6418ae6636" },
    GeneratedContractArtifact { path: "registry/enum-registry.yaml", canonical_digest: "b3:be7a300883f71448cd67f9e5656a42c747d61d97910ac3d9244228c18ede61c1" },
    GeneratedContractArtifact { path: "registry/flag-registry.yaml", canonical_digest: "b3:babc3d8a3c30ccfbf78060290b4a92d979a4f412e730c06065d475fc02ea87b8" },
    GeneratedContractArtifact { path: "registry/ontology-entity-registry.yaml", canonical_digest: "b3:82d592e3a72abb684bba1561e7cfc3ad9e88acabc7bf549b1904715decd9d700" },
    GeneratedContractArtifact { path: "registry/ontology-relation-registry.yaml", canonical_digest: "b3:c3cb240a66c59bc78b37a41a960d48d00d7ab517030470aa8093327f3ebb82b5" },
    GeneratedContractArtifact { path: "registry/ontology-property-registry.yaml", canonical_digest: "b3:db9e89f8184e21af41168e54201d9e48d04c4001fb8e8ee1df730026adc59026" },
    GeneratedContractArtifact { path: "registry/unknown-registry.yaml", canonical_digest: "b3:fb9af7dd1ccec61ac21b37e059fb3e9f94644db0ff00f87617665a28b3100819" },
    GeneratedContractArtifact { path: "registry/projection-registry.yaml", canonical_digest: "b3:90f539ada171ccc99d5fefb3745a607ff1f1b59b73a5c7c3a1b13d8ff874b4d6" },
    GeneratedContractArtifact { path: "registry/summary-registry.yaml", canonical_digest: "b3:3d1f9059ae6328804cbf29e91a2568a97e286ff0ef9a566632f10b14039450ce" },
    GeneratedContractArtifact { path: "registry/capability-registry.yaml", canonical_digest: "b3:a83d6467a7fa955a33840174b07320b2d792e7345285b920f0961b7e19064217" },
    GeneratedContractArtifact { path: "registry/error-registry.yaml", canonical_digest: "b3:4e20d6f2702471a6642adcb351672111d6ac670bb4f667d5e2d6eec48412d2f0" },
    GeneratedContractArtifact { path: "registry/provider-registry.yaml", canonical_digest: "b3:13c84fbb02fe15c58bc9e600840c6266a2002f2d3e66325e88c9380301b98b25" },
    GeneratedContractArtifact { path: "registry/derivation-registry.yaml", canonical_digest: "b3:21a0c156ceb7eb9f021e5da7f0e6e03b9c45fd614a8297aadfdadd0c525be55f" },
    GeneratedContractArtifact { path: "registry/phrase-registry.yaml", canonical_digest: "b3:ea2881a1f5df5656e282c660eed619c23d5ab738be95c14df804edad376fee5c" },
    GeneratedContractArtifact { path: "registry/model-pack.schema.json", canonical_digest: "b3:2e7be04d73e766f1b84e3f95c5403f2890e587390ad4d67587108295978c8d27" },
    GeneratedContractArtifact { path: "identity/cbef-v1.yaml", canonical_digest: "b3:f725d42ef087e820cea9c3b24b61c45c673ea6ff3dce6d29cd5577ab2d5b4686" },
    GeneratedContractArtifact { path: "identity/type-algebra-v1.yaml", canonical_digest: "b3:fdb9e4d9a5c17c0a22bbb2372a15c94bfa3b99145cd43a1d2269f5e7c9e5068a" },
    GeneratedContractArtifact { path: "identity/path-canonicalization-v1.yaml", canonical_digest: "b3:2174e8548c83f7ce70181d70c9dd04e0404b945b7ac692ca59e370227243ac43" },
    GeneratedContractArtifact { path: "schema/analysis-context.schema.json", canonical_digest: "b3:842eac4b95855b5cfa1ab084914ab5ca07b62519befd74222226ae21a12777b9" },
    GeneratedContractArtifact { path: "schema/serving-snapshot.schema.json", canonical_digest: "b3:7152838fb0ae262dffee65edd40576c0724c4dbf17250b77e9974e315e0e0141" },
    GeneratedContractArtifact { path: "schema/public-snapshot-metadata.schema.json", canonical_digest: "b3:63c24baab83d2560821c386e862d118068c26253bc230a14c0f6f3c07712f3d1" },
    GeneratedContractArtifact { path: "schema/source-context.schema.json", canonical_digest: "b3:dcf1b47c92703fd63ea4fd327a60a853592da92083ee511475e82d2083749473" },
    GeneratedContractArtifact { path: "schema/cpg-semantic-query-request.schema.json", canonical_digest: "b3:805276c7c2d9302613d9caeade7cf0d07b094e27b0711368f9d16d75637b1c95" },
    GeneratedContractArtifact { path: "schema/cpg-semantic-query-response.schema.json", canonical_digest: "b3:23555764f5a28f433e4eecbfda1dc30ccb5d3c7da30fd087b87a28c477921550" },
    GeneratedContractArtifact { path: "schema/public-status.schema.json", canonical_digest: "b3:527ce9b336ed86292cd03921803f66dec4c3a21fafb871e1258262c063c7e4b6" },
    GeneratedContractArtifact { path: "query/english-controlled-v1.ebnf", canonical_digest: "b3:be0fcd6821962b475ea8291f7b4d5a66bcfe527d94831d50dd8c40900d4738c5" },
    GeneratedContractArtifact { path: "query/planspec.schema.json", canonical_digest: "b3:6e77244f195f755a4c22eea3fc89279c57e847978a9d47bf454830dbf3771f65" },
    GeneratedContractArtifact { path: "rpc/cpg_query_service.proto", canonical_digest: "b3:44c8ee84a468fe859a85392e842bddae3a5e983a402bade1cbd408e9916097bf" },
    GeneratedContractArtifact { path: "rpc/provider_control.proto", canonical_digest: "b3:0d5e0594077ea6a4e402abde6fa09ea2170d0df3bd937a7d993e452a012578d2" },
    GeneratedContractArtifact { path: "rpc/pyrefly_sidecar.proto", canonical_digest: "b3:b1550fd5f589f06b68a5ae243efa3bb79d9e5ffb33f8b8721bf61bc71daea2c0" },
    GeneratedContractArtifact { path: "rpc/rustc_extractor.proto", canonical_digest: "b3:7d6b422f512d76be14b3cf01251396557fc6b6265998cf628a68dd827fd923ca" },
    GeneratedContractArtifact { path: "rpc/feature-registry.yaml", canonical_digest: "b3:824719acaa747327b7a76745cfb0d3186db631bde4c039621077c27d10e51f4f" },
    GeneratedContractArtifact { path: "adapter/fastmcp-input.schema.json", canonical_digest: "b3:2935eac4143de8ba3b35afb7275f04c68dbec2482981e55fa206384c1a0d4101" },
    GeneratedContractArtifact { path: "adapter/fastmcp-output.schema.json", canonical_digest: "b3:eb4727e15e773863155b140be88b44dc70d7fa642d900edee4efe9cba0222c3f" },
    GeneratedContractArtifact { path: "adapter/fastmcp-public-meta.schema.json", canonical_digest: "b3:c42d342737660595204db8f0c3847e918472312b0682d011eb95a1e27f8cd199" },
    GeneratedContractArtifact { path: "bundles/ontology-bundle.json", canonical_digest: "b3:71d18c359a0e6e89254257292f29d6261b929f5bdeb660d649fe3885e0a75b45" },
    GeneratedContractArtifact { path: "bundles/schema-bundle.json", canonical_digest: "b3:f92fa46d969a7277415c0fef93cc215597795a890c1ff1a0baabb0a90b50c0d2" },
    GeneratedContractArtifact { path: "bundles/provider-bundle.json", canonical_digest: "b3:a1dc59a034baf9ddedb792dfc2d790bd4e9390d3e75a815122182dd82a4a477d" },
    GeneratedContractArtifact { path: "bundles/derivation-bundle.json", canonical_digest: "b3:8fadff9b42573de33887d832d37dd6bc59dc26611a1d8d6dbbce80f42da826be" },
    GeneratedContractArtifact { path: "bundles/query-language-bundle.json", canonical_digest: "b3:60186f71316c20c8ef48b8d57a79c214d525bf9781d1ac5e1eae1491c79a223a" },
    GeneratedContractArtifact { path: "bundles/tool-contract-bundle.json", canonical_digest: "b3:d4bdabd8ff8be7b46115c4072ae076c590cdd64a79791ce21ad64547296359f3" },
    GeneratedContractArtifact { path: "bundles/toolchain-bundle.json", canonical_digest: "b3:93ab4a6c4a2801cce11f8ebec86e0faaa8022840631c427e55f6330e79836e80" },
    GeneratedContractArtifact { path: "bundles/model-pack-bundle.json", canonical_digest: "b3:e5a8d35c9e6544f6cffdd4ae1531385d922489d263947e747061c53c697b6fa9" },
    GeneratedContractArtifact { path: "deployment/local-workstation-v1.yaml", canonical_digest: "b3:74dd1252c43be85e3991f4dad257d3ab4d50c60bb5f56419155ed7bcf2278033" },
    GeneratedContractArtifact { path: "faults/fault-point-registry.yaml", canonical_digest: "b3:6a14233a7b909bef2f7e8e9118e2ecf141582dbc930bd827ab08abe82662c26e" },
    GeneratedContractArtifact { path: "comparison/comparison-ignore-registry.yaml", canonical_digest: "b3:ef9336890222a75017dde012302db7fadb7eaeff0b49d7b0acf40904570a95fa" },
    GeneratedContractArtifact { path: "security/security-corpus-manifest.yaml", canonical_digest: "b3:3866a9122321481276cc720f531f41c3dbdf989460d182522c0437ca163f731a" },
];
