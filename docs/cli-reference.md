# CLI Reference

```sh
$ snapvrt --help
Visual regression testing tool

Usage: snapvrt <COMMAND>

Commands:
  init     Initialize a new snapvrt project
  test     Run visual regression tests
  update   Capture and save reference snapshots
  approve  Approve pending snapshot changes
  review   Launch interactive review UI
  prune    Remove orphaned reference snapshots
  service  Manage the HTTP API service

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```sh
$ snapvrt init --help
Initialize a new snapvrt project

Usage: snapvrt init [SOURCE] [OPTIONS]

Arguments:
  [SOURCE]  Source to add: storybook, pdf, web (omit for minimal config)

Options:
  -h, --help  Print help

Behavior:
  - Creates .snapvrt/ if it doesn't exist
  - Adds source config without overwriting existing sources
  - Safe to run multiple times

Examples:
  snapvrt init              # Create minimal config
  snapvrt init storybook    # Add Storybook source
  snapvrt init pdf          # Add PDF source
```

```sh
$ snapvrt test --help
Run visual regression tests

Usage: snapvrt test [SOURCE] [OPTIONS]

Arguments:
  [SOURCE]  Source to test: storybook, pdf, web (omit for all configured sources)

Options:
      --filter <PATTERN>  Filter snapshots by name pattern
      --auto-add-new      Automatically approve new snapshots
      --prune             Delete orphaned reference snapshots
      --fail-fast         Stop on first source error
  -h, --help              Print help

Source-specific options (require explicit source):
  Storybook:
      --url <URL>              Storybook server URL
      --static-dir <PATH>      Path to static Storybook build

  PDF:
      --manifest <PATH>        Path to PDF manifest file
      --dpi <DPI>              Render resolution [default: 144]

  Web:
      --manifest <PATH>        Path to web manifest file
      --viewport <VIEWPORT>    Viewport to use
```

```sh
$ snapvrt update --help
Capture and save reference snapshots directly (no comparison)

Usage: snapvrt update [SOURCE] [OPTIONS]

Arguments:
  [SOURCE]  Source to update: storybook, pdf, web (omit for all configured sources)

Options:
      --filter <PATTERN>  Filter snapshots by name pattern
  -h, --help              Print help
```

```sh
$ snapvrt approve --help
Approve pending snapshot changes (save current as reference)

Usage: snapvrt approve [SOURCE] [OPTIONS]

Arguments:
  [SOURCE]  Source to approve from: storybook, pdf, web [default: all]

Options:
      --filter <PATTERN>  Approve snapshots matching pattern
      --new               Approve only new snapshots
      --failed            Approve only failed snapshots
      --all               Approve all pending (new + failed)
  -h, --help              Print help

Examples:
  snapvrt approve --filter button-primary
  snapvrt approve --filter "button-*"
  snapvrt approve storybook --all
  snapvrt approve --new
```

```sh
$ snapvrt review --help
Launch interactive review UI

Usage: snapvrt review [SOURCE] [OPTIONS]

Arguments:
  [SOURCE]  Source to review: storybook, pdf, web [default: all]

Options:
  -h, --help  Print help
```

```sh
$ snapvrt prune --help
Remove orphaned reference snapshots

Usage: snapvrt prune [SOURCE] [OPTIONS]

Arguments:
  [SOURCE]  Source to prune: storybook, pdf, web [default: all]

Options:
      --yes      Skip confirmation prompt
      --dry-run  Show what would be deleted
  -h, --help     Print help
```

```sh
$ snapvrt service --help
Manage the HTTP API service

Usage: snapvrt service <COMMAND>

Commands:
  start   Start the HTTP API service
  stop    Stop the running service
  status  Show service status and pending diffs

Options:
  -h, --help  Print help
```

## Workflows

### `update` vs `test` + `approve`

| Command   | When to use                  | What it does                         |
| --------- | ---------------------------- | ------------------------------------ |
| `update`  | Intentional baseline refresh | Capture → save directly as reference |
| `test`    | CI / verification            | Capture → save as current → compare  |
| `approve` | After `test` found changes   | Save current as reference            |

Use `update` when you intentionally want new baselines (e.g., after redesigning a component).
Use `test` + `approve` when you want to review changes first.
