# CLI Reference

## Commands

```
snapvrt <COMMAND>

Commands:
  init      Create .snapvrt/ with default config
  test      Discover, capture, compare, and report visual differences
  update    Discover, capture, and save as reference snapshots
  approve   Promote current/ snapshots to reference/
  review    Generate a visual review report (static HTML)
  prune     Delete orphaned reference snapshots
```

### `snapvrt init`

Create `.snapvrt/config.toml` with default settings and a `.gitignore`.

```
snapvrt init [OPTIONS]

Options:
      --url <URL>     Storybook URL [default: http://localhost:6006]
  -f, --force         Overwrite existing config and gitignore
```

Safe to run multiple times (won't overwrite unless `--force`).

### `snapvrt test`

Discover stories, capture screenshots, compare against references, and report results.

```
snapvrt test [OPTIONS]

Options:
      --url <URL>                Storybook URL (overrides config)
  -f, --filter <PATTERN>         Only run snapshots whose name contains PATTERN (case-insensitive)
      --threshold <FLOAT>        Max allowed diff score, 0.0-1.0 (overrides config)
      --timings                  Print per-snapshot timing breakdown table
      --prune                    Delete orphaned reference snapshots
      --screenshot <MODE>        Screenshot strategy: stable, single [default: stable]
      --stability-attempts <N>   Max screenshots for stability check [default: 3]
      --stability-delay-ms <MS>  Delay between stability attempts [default: 100]
  -p, --parallel <N>             Concurrent browser tabs [default: 4]
      --chrome-url <URL>         Connect to remote Chrome (e.g. http://localhost:9222)
```

**Exit codes:**

- `0` — all snapshots passed
- `1` — any failures, new snapshots, or capture errors

### `snapvrt update`

Discover stories, capture screenshots, and save directly as reference snapshots (no comparison).

```
snapvrt update [OPTIONS]

Options:
      --url <URL>                Storybook URL (overrides config)
  -f, --filter <PATTERN>         Only run snapshots whose name contains PATTERN (case-insensitive)
      --timings                  Print per-snapshot timing breakdown table
      --screenshot <MODE>        Screenshot strategy: stable, single
      --stability-attempts <N>   Max screenshots for stability check
      --stability-delay-ms <MS>  Delay between stability attempts
  -p, --parallel <N>             Concurrent browser tabs
      --chrome-url <URL>         Connect to remote Chrome
```

### `snapvrt approve`

Promote `current/` snapshots to `reference/` without re-capturing. Run after `snapvrt test` to accept changes.

```
snapvrt approve [OPTIONS]

Options:
  -f, --filter <PATTERN>  Only approve snapshots whose name contains PATTERN
      --new               Only approve new snapshots (no prior reference)
      --failed            Only approve failed snapshots (have a diff)
      --all               Approve all pending (new + failed)
```

When no kind flags (`--new`, `--failed`, `--all`) are given, `--all` is the default.

**Examples:**

```sh
snapvrt approve --all                    # Approve everything
snapvrt approve -f button                # Approve snapshots matching "button"
snapvrt approve --new                    # Approve only new snapshots
snapvrt approve --failed -f "card"       # Approve failed snapshots matching "card"
```

### `snapvrt review`

Generate a static HTML report showing reference, current, and diff images side by side.

```
snapvrt review [OPTIONS]

Options:
      --open                  Open the report in the default browser
  -f, --filter <PATTERN>      Only report on snapshots whose name contains PATTERN
      --source <SOURCE>       Only report on the named config source(s); repeatable
```

The report is built from what is on disk in `reference/`, `current/` and
`difference/`, so an unfiltered report after a filtered run also shows rows left
over from earlier runs. Pass the same `-f` you tested with to scope it:

```sh
snapvrt test -f checkout
snapvrt review -f checkout --open
```

`snapvrt test --review` already does this for you — it passes the run's own
filter to the report. A filtered report says so in its header, so it cannot be
mistaken for a clean full run.

### `snapvrt prune`

Find and delete reference snapshots that no longer match any story in the current Storybook.

```
snapvrt prune [OPTIONS]

Options:
      --url <URL>          Storybook URL (overrides config)
      --dry-run            Show what would be deleted without deleting
  -y, --yes                Skip confirmation prompt
      --chrome-url <URL>   Connect to remote Chrome
  -p, --parallel <N>       Concurrent browser tabs
```

## Workflows

### `update` vs `test` + `approve`

| Command   | When to use                  | What it does                         |
| --------- | ---------------------------- | ------------------------------------ |
| `update`  | Intentional baseline refresh | Capture → save directly as reference |
| `test`    | CI / verification            | Capture → compare → report           |
| `approve` | After `test` found changes   | Promote current → reference          |

Use `update` when you intentionally want new baselines (e.g., after a redesign).

Use `test` + `approve` when you want to review changes before accepting them.

### Typical CI workflow

```sh
snapvrt test                     # exits 1 if any diffs or new snapshots
```

### Typical dev workflow

```sh
snapvrt test                     # see what changed
snapvrt review --open            # visual review in browser
snapvrt approve --all            # accept changes
snapvrt test                     # verify — should exit 0
```

### Working on one component

Keep the same `-f` across all four steps, so each command sees the same set and
the report can't show you stale rows from elsewhere in the suite:

```sh
snapvrt test -f checkout --review
snapvrt review -f checkout --open
snapvrt approve -f checkout
snapvrt test -f checkout
```

## Environment Variables

| Variable                 | Description                                                 |
| ------------------------ | ----------------------------------------------------------- |
| `SNAPVRT_STORYBOOK_URL`  | Override Storybook URL (lower priority than `--url`)        |
| `SNAPVRT_DIFF_THRESHOLD` | Override diff threshold (lower priority than `--threshold`) |
| `RUST_LOG`               | Log level filter (e.g. `debug`, `snapvrt=trace`)            |
