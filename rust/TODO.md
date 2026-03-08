# Follow-ups

## Typst version pinning

Research whether snapvrt should enforce or record the Typst CLI version used for rendering.

Typst rendering output may differ between versions (font metrics, layout algorithm changes), which would cause false-positive diffs. Options to consider:

- **Record version in metadata**: run `typst --version` during `update`, store in `.snapvrt/metadata.toml`, warn on `test` if version differs.
- **Config field**: add optional `typst_version = "0.13"` to `[source.typst]`; fail fast if the installed version doesn't match.
- **Bundle typst as a library**: use `typst` crate directly instead of shelling out. Eliminates the external dependency entirely and pins the version via `Cargo.lock`. Trade-off: larger binary, must track upstream API changes.
- **Do nothing**: document that users should keep Typst version consistent across dev/CI and leave it to them.
