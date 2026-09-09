from tooling.ci.repository_checks import local_link_errors


def test_local_links_resolve_from_document_and_ignore_examples(tmp_path):
    document = tmp_path / "notes.md"
    document.write_text(
        "[missing](missing.md)\n```md\n[example](example.md)\n```\n[web](https://example.com)"
    )
    assert len(local_link_errors(document)) == 1
    (tmp_path / "missing.md").touch()
    assert local_link_errors(document) == []
