import shutil
from pathlib import Path

import pytest

from tooling.ci.authoritative_design_conformance import (
    GOVERNANCE,
    ROOT,
    AuthoritativeDesignError,
    selected_documents,
)


def test_selected_documents_do_not_need_a_plan_pointer():
    assert len(selected_documents()) == 8


def test_duplicate_and_missing_links_fail(tmp_path: Path):
    destination = tmp_path / GOVERNANCE.parent
    shutil.copytree(ROOT / GOVERNANCE.parent, destination)
    governance = tmp_path / GOVERNANCE
    original = governance.read_text()
    row = next(line for line in original.splitlines() if line.startswith("| ONT |"))
    governance.write_text(original + "\n" + row + "\n")
    with pytest.raises(AuthoritativeDesignError, match="duplicate"):
        selected_documents(tmp_path)
    governance.write_text(original)
    selected_documents(tmp_path)["ONT"].unlink()
    with pytest.raises(AuthoritativeDesignError, match="invalid selected"):
        selected_documents(tmp_path)
