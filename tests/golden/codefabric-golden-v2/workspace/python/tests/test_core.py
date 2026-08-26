from golden_pkg.core import normalized_total


def test_normalized_total() -> None:
    assert normalized_total([3, 1, 2]) == 6
