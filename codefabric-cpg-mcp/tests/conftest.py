"""Shared adapter test configuration."""

from collections.abc import Iterator

import pytest

from codefabric_cpg_mcp.settings import process_settings

SETTING_ENV_NAMES = (
    "CODEFABRIC_CPG_DAEMON_TARGET",
    "CODEFABRIC_DAEMON_TARGET",
    "CODEFABRIC_WORKSPACE_ID",
    "CODEFABRIC_AGENT_INSTANCE_ID",
    "CODEFABRIC_CPG_CAPABILITY_TOKEN",
    "CODEFABRIC_CPG_QUERY_TIMEOUT_SECONDS",
    "CODEFABRIC_CPG_INLINE_RESULT_BYTES",
    "CODEFABRIC_CPG_MAX_REQUEST_BYTES",
    "CODEFABRIC_CPG_MAX_JSON_DEPTH",
    "CODEFABRIC_CPG_MAX_JSON_NODES",
    "CODEFABRIC_CPG_MAX_VALIDATION_ERRORS",
    "CODEFABRIC_CPG_RESULT_TTL_SECONDS",
    "CODEFABRIC_CPG_LOG_LEVEL",
)


@pytest.fixture(autouse=True)
def clean_adapter_environment(monkeypatch: pytest.MonkeyPatch) -> Iterator[None]:
    """Prevent workstation configuration from affecting settings tests."""

    for name in SETTING_ENV_NAMES:
        monkeypatch.delenv(name, raising=False)
    process_settings.cache_clear()
    yield
    process_settings.cache_clear()
