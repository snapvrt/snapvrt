# Configuration

## Config File

`.snapvrt/config.toml` — created by `snapvrt init`.

```toml
# .snapvrt/config.toml

# ─────────────────────────────────────────────────────────
# Source: Storybook
# ─────────────────────────────────────────────────────────
[source.storybook]
type = "storybook"
url = "http://localhost:6006"
# viewports = ["laptop"]           # optional: omit = use all defined viewports

# ─────────────────────────────────────────────────────────
# Viewports
# ─────────────────────────────────────────────────────────
[viewport.laptop]
width = 1366
height = 768

# [viewport.mobile]
# width = 375
# height = 812

# ─────────────────────────────────────────────────────────
# Capture pipeline — all fields optional
# ─────────────────────────────────────────────────────────
[capture]
# screenshot = "stable"             # "stable" | "single" (single is faster, less reliable)
# stability_attempts = 3            # max screenshots for stable mode
# stability_delay_ms = 100          # delay between stability attempts (ms)
# parallel = 4                      # concurrent browser tabs
# chrome_url = "http://localhost:9222"  # connect to remote Chrome instead of launching one

# ─────────────────────────────────────────────────────────
# Comparison
# ─────────────────────────────────────────────────────────
[diff]
# threshold = 0.0                   # max allowed diff score (0.0 = exact, 0.01 = 1%)
```

## Options

### Source

| Option                    | Required | Default | Description                                        |
| ------------------------- | -------- | ------- | -------------------------------------------------- |
| `source.<name>.type`      | yes      | —       | Source type. Currently only `storybook`            |
| `source.<name>.url`       | yes      | —       | Storybook dev server URL                           |
| `source.<name>.viewports` | no       | all     | Subset of defined viewports to use for this source |

### Viewports

| Option                   | Required | Default | Description                   |
| ------------------------ | -------- | ------- | ----------------------------- |
| `viewport.<name>.width`  | yes      | —       | Viewport width in CSS pixels  |
| `viewport.<name>.height` | yes      | —       | Viewport height in CSS pixels |

If no viewports are defined, a default `laptop` viewport (1366x768) is used.

### Capture

| Option                       | Default    | Description                                                                                                 |
| ---------------------------- | ---------- | ----------------------------------------------------------------------------------------------------------- |
| `capture.screenshot`         | `"stable"` | Screenshot strategy: `stable` (up to 3 shots, picks first consecutive match) or `single` (one shot, faster) |
| `capture.stability_attempts` | `3`        | Max screenshots for stable mode                                                                             |
| `capture.stability_delay_ms` | `100`      | Delay between stability attempts in milliseconds                                                            |
| `capture.parallel`           | `4`        | Number of concurrent browser tabs                                                                           |
| `capture.chrome_url`         | —          | Connect to remote Chrome DevTools URL instead of launching a local Chrome process                           |

### Diff

| Option           | Default | Description                                                                                  |
| ---------------- | ------- | -------------------------------------------------------------------------------------------- |
| `diff.threshold` | `0.0`   | Max allowed diff score. `0.0` = exact pixel match required. `0.01` = 1% of pixels can differ |

## Override Precedence

Highest to lowest:

1. CLI flags (`--url`, `--threshold`, `--parallel`, etc.)
2. Environment variables (`SNAPVRT_STORYBOOK_URL`, `SNAPVRT_DIFF_THRESHOLD`)
3. Config file (`.snapvrt/config.toml`)
4. Defaults

## Validation

The config is validated on load. Errors include:

- No sources configured
- No viewports configured
- Viewport with zero width or height
- Source references a viewport that doesn't exist
- Threshold outside 0.0-1.0 range
