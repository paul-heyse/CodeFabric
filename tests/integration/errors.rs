use codefabric::{Error, normalize_workspace_id};

#[test]
fn blank_workspace_id_is_rejected() {
    let error = normalize_workspace_id(" \t ").expect_err("blank input");
    assert_eq!(
        error,
        Error::EmptyField {
            field: "workspace_id"
        }
    );
}

#[test]
fn error_display_is_stable() {
    let error = normalize_workspace_id("").expect_err("empty input");
    assert_eq!(error.to_string(), "workspace_id must not be empty");
}
