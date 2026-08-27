from golden_pkg.core import pipeline


def test_pipeline() -> None:
    assert pipeline(3) == 7
