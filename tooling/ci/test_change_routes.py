import pytest

from tooling.ci.change_routes import DOMAINS, routes


def test_docs_do_not_compile_product():
    actual = routes(["docs/plans/current.md", "AGENTS.md"])
    assert actual["tooling"] and not any(actual[name] for name in DOMAINS)


@pytest.mark.parametrize(
    "path",
    [
        "Cargo.lock",
        "contracts/proto/cpg.proto",
        ".github/workflows/ci.yml",
        "scripts/repo-shell.sh",
        "third_party/native/arrow/src/lib.rs",
        "unclassified",
    ],
)
def test_shared_inputs_select_all_consumers(path):
    assert all(routes([path]).values())


def test_domain_and_scheduled_routes():
    actual = routes(["rustc-extractor/src/main.rs"])
    assert actual["extractor"] and actual["contracts"] and not actual["sidecar"]
    assert all(routes([], full=True).values())
