use std::fs;
use std::path::PathBuf;

const PYREFLY_VERSION: &str = "1.2.0";
const PYREFLY_COMMIT: &str = "1933169ad8ee9e4d4114112eb56ef0811fb0a094";
const EXPECTED_SOURCE: &str = "git+https://github.com/facebook/pyrefly?rev=1933169ad8ee9e4d4114112eb56ef0811fb0a094#1933169ad8ee9e4d4114112eb56ef0811fb0a094";
const LSP_TYPES_VERSION: &str = "0.95.2";
const EXPECTED_LSP_TYPES_SOURCE: &str = "git+https://github.com/yangdanny97/lsp-types?rev=395d6bfcd6c3696a64cfe9cd93b86f981fb85112#395d6bfcd6c3696a64cfe9cd93b86f981fb85112";

fn locked_package_source<'a>(
    lockfile: &'a str,
    package_name: &str,
    package_version: &str,
) -> Option<&'a str> {
    lockfile.split("[[package]]").skip(1).find_map(|package| {
        let mut name = None;
        let mut version = None;
        let mut source = None;
        for line in package.lines().map(str::trim) {
            if let Some(value) = line
                .strip_prefix("name = \"")
                .and_then(|v| v.strip_suffix('"'))
            {
                name = Some(value);
            } else if let Some(value) = line
                .strip_prefix("version = \"")
                .and_then(|v| v.strip_suffix('"'))
            {
                version = Some(value);
            } else if let Some(value) = line
                .strip_prefix("source = \"")
                .and_then(|v| v.strip_suffix('"'))
            {
                source = Some(value);
            }
        }
        (name == Some(package_name) && version == Some(package_version))
            .then_some(source)
            .flatten()
    })
}

fn main() {
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let lock_path = manifest_dir.join("Cargo.lock");
    let identity_path = manifest_dir.join("toolchain-identity.json");
    println!("cargo:rerun-if-changed={}", lock_path.display());
    println!("cargo:rerun-if-changed={}", identity_path.display());

    let lockfile = fs::read_to_string(&lock_path).expect("read sidecar Cargo.lock");
    let source = locked_package_source(&lockfile, "pyrefly", PYREFLY_VERSION)
        .expect("find pyrefly 1.2.0 in Cargo.lock");
    assert_eq!(
        source, EXPECTED_SOURCE,
        "Pyrefly lock source or commit drifted"
    );
    assert!(source.ends_with(PYREFLY_COMMIT));
    let lsp_types_source = locked_package_source(&lockfile, "lsp-types", LSP_TYPES_VERSION)
        .expect("find lsp-types 0.95.2 in Cargo.lock");
    assert_eq!(
        lsp_types_source, EXPECTED_LSP_TYPES_SOURCE,
        "Pyrefly's transitive lsp-types source or commit drifted"
    );

    let digest = blake3::hash(source.as_bytes()).to_hex().to_string();
    let identity = fs::read_to_string(identity_path).expect("read sidecar identity");
    assert!(
        identity.contains(&digest),
        "toolchain identity does not match the locked Pyrefly source digest"
    );
    println!("cargo:rustc-env=PYREFLY_LOCK_SOURCE_BLAKE3={digest}");
}
