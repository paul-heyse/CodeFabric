# Shared cross-language fixtures

Every fixture beneath this directory is immutable test input shared by at least
two build domains. Tests must copy stateful inputs into a per-test temporary
directory; they must never mutate this authority copy.
