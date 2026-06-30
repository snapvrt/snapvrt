# Follow-ups

## Typst version pinning

Research whether snapvrt should enforce or record the Typst CLI version used for rendering.

Typst rendering output may differ between versions (font metrics, layout algorithm changes), which would cause false-positive diffs. Options to consider:

- **Record version in metadata**: run `typst --version` during `update`, store in `.snapvrt/metadata.toml`, warn on `test` if version differs.
- **Config field**: add optional `typst_version = "0.13"` to `[source.typst]`; fail fast if the installed version doesn't match.
- **Bundle typst as a library**: use `typst` crate directly instead of shelling out. Eliminates the external dependency entirely and pins the version via `Cargo.lock`. Trade-off: larger binary, must track upstream API changes.
- **Do nothing**: document that users should keep Typst version consistent across dev/CI and leave it to them.

## Typst `data.json` race condition risk

Templates read fixture data via `json("data.json")`, which Typst resolves relative to the `.typ` file's directory. Currently snapvrt copies the fixture JSON to `data.json` next to the template before compiling, then deletes it via an RAII guard.

This is safe today because Typst templates are compiled **sequentially** in `plan_typst`. But if compilation is ever parallelized, two fixtures in the same directory would race on the same `data.json` file.

Options:

- **`sys.inputs` approach**: pass the data path via `--input data-file=/tmp/data-{uuid}.json`. Templates change their top line to `json(sys.inputs.at("data-file", default: "data.json"))`. Unique temp file per compile, no writes next to templates, backward compatible (falls back to `data.json` for standalone use).
- **Use `typst-renderer` as a library**: compile in-process with a virtual filesystem (like the LIMS `LimsWorld` implementation). Eliminates the temp file entirely. Trade-off: heavier dependency, must replicate font loading.
- **Temp directory per compile**: copy/symlink the template tree into a temp dir with its own `data.json`. Heavy on I/O for large template trees.
