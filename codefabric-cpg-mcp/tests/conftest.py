"""Shared isolation for the inherited launch-authority channel."""

from collections.abc import Iterator

import pytest

import codefabric_cpg_mcp.settings as settings_module
from codefabric_cpg_mcp.settings import process_settings


@pytest.fixture(autouse=True)
def clean_launch_authority(monkeypatch: pytest.MonkeyPatch) -> Iterator[None]:
    """Never allow one test's retained fd3 stream to authorize another."""

    process_settings.cache_clear()
    monkeypatch.setattr(settings_module, "_launch_socket", None)
    settings_module._launch_buffer.clear()
    yield
    process_settings.cache_clear()
    retained = settings_module._launch_socket
    if retained is not None:
        retained.close()
    monkeypatch.setattr(settings_module, "_launch_socket", None)
    settings_module._launch_buffer.clear()
