# Configuration

## Config File

`.snapvrt/config.toml`

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

[viewport.mobile]
width = 375
height = 812

# ─────────────────────────────────────────────────────────
# Capture pipeline — all fields optional.
# ─────────────────────────────────────────────────────────
[capture]
# preset = "standard"               # "standard" | "loki"
# animation = "post-load"           # "post-load" | "loki"
# clip = "story-root"               # "story-root" | "body"
# screenshot = "stable"             # "stable" | "single" (single is faster)
# stability_attempts = 3
# stability_delay_ms = 100
# network_wait = "idle"             # "none" | "idle" | "fixed"
# network_wait_delay_ms = 500       # for "fixed" variant
# parallel = 4                      # concurrent browser tabs
# chrome_url = "http://localhost:9222"  # remote Chrome (e.g. Docker)

# ─────────────────────────────────────────────────────────
# Comparison
# ─────────────────────────────────────────────────────────
[diff]
# threshold = 0.0                   # max allowed diff score (0.0 = exact, 0.01 = 1%)
```

### Multi-source Example

```toml
[source.design-system]
type = "storybook"
url = "http://localhost:6006"
viewports = ["laptop", "mobile"]

[source.docs]
type = "storybook"
url = "http://localhost:6007"
viewports = ["laptop"]

[viewport.laptop]
width = 1366
height = 768

[viewport.mobile]
width = 375
height = 812
```

## Options

### Source

| Option                    | Required | Default | Description                                        |
| ------------------------- | -------- | ------- | -------------------------------------------------- |
| `source.<name>.type`      | yes      | -       | Source type (`storybook`)                          |
| `source.<name>.url`       | yes      | -       | Storybook dev server URL                           |
| `source.<name>.viewports` | no       | all     | Subset of defined viewports to use for this source |

### Viewports

| Option                   | Required | Default | Description                   |
| ------------------------ | -------- | ------- | ----------------------------- |
| `viewport.<name>.width`  | yes      | -       | Viewport width in CSS pixels  |
| `viewport.<name>.height` | yes      | -       | Viewport height in CSS pixels |

### Capture

| Option                          | Default       | Description                                                  |
| ------------------------------- | ------------- | ------------------------------------------------------------ |
| `capture.preset`                | `"standard"`  | Base preset (`standard`, `loki`)                             |
| `capture.animation`             | (from preset) | Animation handling (`post-load`, `loki`)                     |
| `capture.clip`                  | (from preset) | Clip region calculation (`story-root`, `body`)               |
| `capture.screenshot`            | (from preset) | Screenshot strategy (`stable`, `single`); `single` is faster |
| `capture.stability_attempts`    | `3`           | Max attempts for stable screenshot comparison                |
| `capture.stability_delay_ms`    | `100`         | Delay between stability attempts in milliseconds             |
| `capture.network_wait`          | (from preset) | Network settling strategy (`none`, `idle`, `fixed`)          |
| `capture.network_wait_delay_ms` | `500`         | Delay for `fixed` network wait in milliseconds               |
| `capture.parallel`              | `4`           | Concurrent browser tabs for capturing                        |
| `capture.chrome_url`            | -             | Remote Chrome DevTools URL (e.g. Docker)                     |

### Diff

| Option           | Default | Description                                |
| ---------------- | ------- | ------------------------------------------ |
| `diff.threshold` | `0.0`   | Max allowed diff score (0.0 = exact match) |

## Override Precedence

Highest to lowest:

1. CLI flags (`--url http://localhost:6006`)
2. Environment variables (`SNAPVRT_STORYBOOK_URL`)
3. Config file (`.snapvrt/config.toml`)
4. Defaults
