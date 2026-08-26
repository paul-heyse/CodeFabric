"""Settings contract tests for SRV section 55."""

from pathlib import Path

import pytest
from pydantic import ValidationError

from codefabric_cpg_mcp.settings import Settings, process_settings


def required_settings(**overrides: object) -> Settings:
    values: dict[str, object] = {
        "daemon_target": "unix:///tmp/codefabric.sock",
        "workspace_id": "workspace-main",
        "agent_instance_id": "agent-primary",
        "capability_token": "test-secret",
    }
    values.update(overrides)
    return Settings(**values)  # type: ignore[arg-type]


def test_required_values_are_enforced() -> None:
    with pytest.raises(ValidationError) as error:
        Settings()

    rendered = str(error.value)
    assert "CODEFABRIC_CPG_DAEMON_TARGET" in rendered
    assert "CODEFABRIC_WORKSPACE_ID" in rendered
    assert "CODEFABRIC_CPG_CAPABILITY_TOKEN" in rendered


def test_environment_values_are_converted(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("CODEFABRIC_CPG_DAEMON_TARGET", "tcp://127.0.0.1:50051")
    monkeypatch.setenv("CODEFABRIC_WORKSPACE_ID", "workspace-main")
    monkeypatch.setenv("CODEFABRIC_AGENT_INSTANCE_ID", "agent-primary")
    monkeypatch.setenv("CODEFABRIC_CPG_CAPABILITY_TOKEN", "test-secret")
    monkeypatch.setenv("CODEFABRIC_CPG_QUERY_TIMEOUT_SECONDS", "4.5")
    monkeypatch.setenv("CODEFABRIC_CPG_MAX_JSON_NODES", "250")

    settings = Settings()

    assert settings.query_timeout_seconds == 4.5
    assert settings.max_json_nodes == 250


def test_process_settings_is_one_instance_per_process(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("CODEFABRIC_CPG_DAEMON_TARGET", "unix:///tmp/codefabric.sock")
    monkeypatch.setenv("CODEFABRIC_WORKSPACE_ID", "workspace-main")
    monkeypatch.setenv("CODEFABRIC_AGENT_INSTANCE_ID", "agent-primary")
    monkeypatch.setenv("CODEFABRIC_CPG_CAPABILITY_TOKEN", "test-secret")

    first = process_settings()
    monkeypatch.setenv("CODEFABRIC_WORKSPACE_ID", "replacement-workspace")

    assert process_settings() is first
    assert process_settings().workspace_id == "workspace-main"


def test_canonical_daemon_alias_precedes_migration_alias(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("CODEFABRIC_CPG_DAEMON_TARGET", "unix:///canonical.sock")
    monkeypatch.setenv("CODEFABRIC_DAEMON_TARGET", "unix:///migration.sock")
    monkeypatch.setenv("CODEFABRIC_WORKSPACE_ID", "workspace-main")
    monkeypatch.setenv("CODEFABRIC_CPG_CAPABILITY_TOKEN", "test-secret")

    assert Settings().daemon_target == "unix:///canonical.sock"


def test_migration_daemon_alias_is_accepted(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("CODEFABRIC_DAEMON_TARGET", "unix:///migration.sock")
    monkeypatch.setenv("CODEFABRIC_WORKSPACE_ID", "workspace-main")
    monkeypatch.setenv("CODEFABRIC_CPG_CAPABILITY_TOKEN", "test-secret")

    assert Settings().daemon_target == "unix:///migration.sock"


def test_constructor_values_precede_environment(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("CODEFABRIC_WORKSPACE_ID", "environment-workspace")

    assert required_settings(workspace_id="constructor-workspace").workspace_id == (
        "constructor-workspace"
    )


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("query_timeout_seconds", 0),
        ("inline_result_bytes", 16 * 1024 - 1),
        ("max_request_bytes", 16 * 1024 * 1024 + 1),
        ("max_json_depth", 129),
        ("max_json_nodes", 99),
        ("max_validation_errors", 101),
        ("result_ttl_seconds", 59),
    ],
)
def test_numeric_ranges_are_enforced(field: str, value: object) -> None:
    with pytest.raises(ValidationError):
        required_settings(**{field: value})


def test_daemon_scheme_is_restricted() -> None:
    with pytest.raises(ValidationError, match="must use unix:// or tcp://"):
        required_settings(daemon_target="https://example.invalid")


def test_settings_are_frozen_and_secrets_are_redacted() -> None:
    settings = required_settings()

    assert "test-secret" not in repr(settings)
    assert settings.model_dump(mode="json")["capability_token"] == "**********"
    with pytest.raises(ValidationError, match="frozen"):
        settings.workspace_id = "replacement"  # type: ignore[misc]


def test_dotenv_is_not_a_settings_source(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    (tmp_path / ".env").write_text(
        "CODEFABRIC_CPG_DAEMON_TARGET=unix:///dotenv.sock\n"
        "CODEFABRIC_WORKSPACE_ID=dotenv-workspace\n"
        "CODEFABRIC_CPG_CAPABILITY_TOKEN=dotenv-secret\n",
        encoding="utf-8",
    )
    monkeypatch.chdir(tmp_path)

    with pytest.raises(ValidationError):
        Settings()
