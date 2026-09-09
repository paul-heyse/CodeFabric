# Selected Pyrefly context fix

Origin: Pyrefly 1.2.0, commit `1933169ad8ee9e4d4114112eb56ef0811fb0a094`.

`Query::make_handle` uses the selected file configuration's `get_sys_info()`.
Upstream Query stored Python 3.13/Linux defaults even when its ConfigFinder selected
another version or platform. This narrow behavior fix lets the sidecar honor its
Python context without copying the Query implementation into application code.
All sibling dependencies remain at the original Git revision. This dependency belongs
only to the existing sidecar build domain. Remove the local fork when a selected upstream
revision supplies the same behavior.

`Query::add_files_with_diagnostic_paths` preserves each diagnostic's exact `ModulePath`
beside its rendered message. The sidecar assigns diagnostics by this owner, including
non-Unicode paths and paths whose display strings collide. The existing `add_files` API
keeps its string-only return for other consumers.
