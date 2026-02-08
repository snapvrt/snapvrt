# Configuration

> TODO: Write configuration reference

## Config File

`.snapvrt/config.toml`

```toml
# .snapvrt/config.toml

# ─────────────────────────────────────────────────────────
# Diff engine
# ─────────────────────────────────────────────────────────
[diff]
engine = "pixelmatch"   # pixelmatch (default), odiff, imagemagick
threshold = 0.0001

# ─────────────────────────────────────────────────────────
# Workers
# ─────────────────────────────────────────────────────────
[workers]
count = 1
tabs_per_worker = 4

# ─────────────────────────────────────────────────────────
# Viewports
# ─────────────────────────────────────────────────────────
[viewports.desktop]
width = 1366
height = 768

[viewports.mobile]
width = 375
height = 667
device_scale_factor = 2

# ─────────────────────────────────────────────────────────
# Source: Storybook
# ─────────────────────────────────────────────────────────
[sources.storybook]
url = "http://localhost:6006"       # Dev server URL
# static_dir = "./storybook-static" # OR static build path
viewports = ["desktop", "mobile"]   # Which viewports to test
exclude_tags = ["snapvrt-skip"]     # Stories to skip

# ─────────────────────────────────────────────────────────
# Source: PDF (batch mode)
# ─────────────────────────────────────────────────────────
# [sources.pdf]
# manifest = "./snapvrt-pdfs.json"
# dpi = 144
# merge = false
```

## Options

### Global

| Option                    | Default        | Description                                        |
| ------------------------- | -------------- | -------------------------------------------------- |
| `diff.engine`             | `"pixelmatch"` | Diff engine (`pixelmatch`, `odiff`, `imagemagick`) |
| `diff.threshold`          | `0.0001`       | Acceptable difference threshold                    |
| `workers.count`           | `1`            | Number of worker containers                        |
| `workers.tabs_per_worker` | `4`            | Concurrent browser tabs per worker                 |

### Viewports

| Option                                 | Default | Description                                    |
| -------------------------------------- | ------- | ---------------------------------------------- |
| `viewports.<name>.width`               | -       | Viewport width in CSS pixels                   |
| `viewports.<name>.height`              | -       | Viewport height in CSS pixels                  |
| `viewports.<name>.device_scale_factor` | `1`     | Pixel density ratio (1 = standard, 2 = retina) |

### Source: Storybook

| Option                           | Default                   | Description                                           |
| -------------------------------- | ------------------------- | ----------------------------------------------------- |
| `sources.storybook.url`          | `"http://localhost:6006"` | Storybook dev server URL                              |
| `sources.storybook.static_dir`   | -                         | Path to static Storybook build (alternative to `url`) |
| `sources.storybook.viewports`    | `["desktop"]`             | Viewports to test                                     |
| `sources.storybook.exclude_tags` | `["snapvrt-skip"]`        | Story tags to skip                                    |

### Source: PDF

| Option                 | Default | Description                   |
| ---------------------- | ------- | ----------------------------- |
| `sources.pdf.manifest` | -       | Path to PDF manifest file     |
| `sources.pdf.dpi`      | `144`   | Default render resolution     |
| `sources.pdf.merge`    | `false` | Stack pages into single image |

## Override Precedence

Highest to lowest:

1. CLI flags (`--url http://localhost:6006`)
2. Environment variables (`SNAPVRT_STORYBOOK_URL`)
3. Config file (`.snapvrt/config.toml`)
4. Defaults
