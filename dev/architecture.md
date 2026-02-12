# Architecture

Status: Active | Date: 2026-02-12

This is the single source of truth for the snapvrt system architecture. For historical context on earlier designs, see `dev/archive/`.

## Overview

snapvrt is a visual regression testing tool for Storybook. It ships as a single Rust binary that:

1. Discovers stories from a running Storybook instance
2. Captures screenshots via Chrome DevTools Protocol (CDP)
3. Compares them against reference snapshots using dify (in-process)
4. Reports results to the terminal and an HTML report

Chrome runs in a Docker container to guarantee consistent rendering. Diffing runs in-process (no container needed). Everything except Chrome is one process.

```
snapvrt test
│
├── 1. Discover stories (HTTP GET index.json from Storybook)
├── 2. Start ghcr.io/snapvrt/chrome container (bollard)  ← NOT YET IMPLEMENTED
│     └── Chrome + pinned fonts, CDP on port 9222
├── 3. Capture screenshots via CDP (N parallel tabs)
├── 4. Compare: memcmp → dify (in-process, spawn_blocking)
└── 5. Report results + write store + stop container
```

**Current state:** Docker integration is not built yet. The CLI either launches a local Chrome process or connects to one at `--chrome-url`. The Docker container management (bollard, auto-start/stop, image pull) is the next major piece of work.

## Why This Design

**Why Docker for Chrome (but not for diffing):**

- Visual regression demands pixel-identical rendering across environments
- A pinned Docker image guarantees exact Chrome version, fonts, and rendering pipeline
- Developer laptops and CI produce byte-identical screenshots
- Diffing is pure math on pixel buffers — deterministic everywhere, no container needed
- dify runs in 44ms in-process; the HTTP serialization overhead would exceed the computation

**Why a single binary (not microservices):**

- The earlier design had 4 crates and separate HTTP services for capture and diff
- With diff running in-process and capture being a direct CDP connection, there's nothing for the services to do
- One crate eliminates protocol versioning, compile-time overhead, and deployment complexity
- If capture/diff ever need to become services again (cloud scaling), the wire protocols from the original spec (archived in `dev/archive/004-protocols.md`) can be revived

## Crate Structure

Single crate: `rust/crates/snapvrt/`

```
src/
├── main.rs              Entry point, command dispatch
├── cli.rs               Clap definitions, arg parsing
├── config/
│   ├── mod.rs           Config struct, TOML parsing, validation
│   ├── capture.rs       CaptureConfig (screenshot strategy, parallelism, chrome_url)
│   ├── resolve.rs       CLI > env > file > defaults merge → ResolvedRunConfig
│   └── template.rs      `init` command config template + gitignore
├── storybook/
│   ├── mod.rs           Story filtering, ID normalization
│   └── discovery.rs     Fetch index.json, parse stories, build iframe URLs
├── cdp/
│   ├── mod.rs
│   ├── chrome.rs        Launch local Chrome or connect to remote, tab create/close
│   └── connection.rs    Per-target WebSocket transport, CDP command/event protocol
├── capture/
│   ├── mod.rs           Re-exports
│   ├── plan.rs          Build job matrix (stories × viewports), apply filters
│   ├── job.rs           CaptureJob definition (url, viewport, snapshot ID)
│   ├── runner.rs        Parallel worker pool, shared queue, error handling
│   ├── pipeline.rs      9-stage capture pipeline (see below)
│   ├── strategy.rs      Screenshot strategy (stable vs single), clip calculation
│   ├── scripts.rs       Injected JS/CSS (animations, ready detection, clip bounds)
│   └── timing.rs        Per-stage timing capture
├── compare/
│   ├── mod.rs           SnapshotStatus enum (Pass/Fail/New/Error)
│   └── diff.rs          2-phase comparison: memcmp → dify, dimension mismatch handling
├── store/
│   └── mod.rs           .snapvrt/ filesystem operations (read/write/list/clean)
├── report/
│   ├── mod.rs
│   ├── terminal.rs      Color output, progress, timing tables, summary
│   └── html.rs          Static HTML report generation
└── commands/
    ├── mod.rs
    ├── init.rs           Create .snapvrt/ with config template and gitignore
    ├── test.rs           Discover → capture → compare → report (exit 0/1)
    ├── update.rs         Discover → capture → save as reference directly
    ├── approve.rs        Promote current/ → reference/ with filters
    ├── review.rs         Generate static HTML report
    └── prune.rs          Delete orphaned references not matching any story
```

## Capture Pipeline

Each screenshot goes through a 9-stage CDP pipeline (`capture/pipeline.rs`):

| Stage                 | What                                                                   | Timeout |
| --------------------- | ---------------------------------------------------------------------- | ------- |
| 1. Set viewport       | `Emulation.setDeviceMetricsOverride`                                   | —       |
| 2. Navigate           | `Page.navigate` to story iframe URL                                    | —       |
| 3. Wait page load     | `Page.loadEventFired`                                                  | —       |
| 4. Network idle       | Track in-flight requests, settle after 100ms quiet                     | 10s     |
| 5. Disable animations | CSS injection + Web Animations API (`animation.finish()`/`.cancel()`)  | —       |
| 6. Wait ready         | `document.fonts.ready` + DOM mutation settle (100ms)                   | 10s     |
| 7. Wait story root    | Poll for `#storybook-root > *` or `#root > *` with non-zero dimensions | 10s     |
| 8. Get clip bounds    | Visible-child-union walk of story root, clamp to viewport              | —       |
| 9. Take screenshot    | `Page.captureScreenshot` with clip; stable mode: up to 3 shots         | —       |

Per-capture timeout: 30s (covers all stages).

**Tall content:** If story height exceeds viewport, the viewport is resized to fit, a 500ms settle delay is applied, and the viewport is restored after capture.

**Parallelism:** N worker tasks (default 4) pull from a shared job queue. Each capture gets a fresh browser tab (avoids WebSocket mutex contention). Results stream via an mpsc channel as they complete.

**Chrome crash detection:** After 3 consecutive session-creation failures, remaining jobs are drained with a crash error.

## 2-Phase Diff Pipeline

```
reference.png bytes == current.png bytes?
├── YES → Pass (unchanged, ~0.02ms)
└── NO  → Decode PNGs → dify perceptual diff (~44ms)
          ├── diff_pixels == 0 → Pass
          ├── score <= threshold → Pass
          └── score > threshold → Fail (write diff.png)
```

dify uses YIQ perceptual color space with anti-aliasing detection. Same algorithm as pixelmatch, implemented natively in Rust. MIT licensed.

**Dimension mismatch:** If reference and current have different sizes, the smaller image is padded to the larger canvas with magenta (#FF00FF). The diff is computed on the padded canvas.

**Score:** `diff_pixels / total_pixels` (0.0 = identical, 1.0 = every pixel differs). Configurable threshold (default 0.0 = exact match).

## Store Layout

```
.snapvrt/
├── config.toml
├── .gitignore
├── reference/                    ← committed (baselines)
│   └── {source}/{viewport}/{title}/{name}.png
├── current/                      ← gitignored (latest capture)
│   └── {source}/{viewport}/{title}/{name}.png
└── difference/                   ← gitignored (diff overlays)
    └── {source}/{viewport}/{title}/{name}.png
```

Example snapshot ID: `storybook/laptop/Example/Button/Primary`

Reference snapshots are committed to git. Current and difference directories are gitignored — they're transient artifacts of a test run.

## Configuration

File: `.snapvrt/config.toml`

```toml
[source.storybook]
type = "storybook"
url = "http://localhost:6006"
# viewports = ["laptop", "mobile"]   # optional: omit = use all

[viewport.laptop]
width = 1366
height = 768

[capture]
# screenshot = "stable"              # "stable" | "single"
# stability_attempts = 3
# stability_delay_ms = 100
# parallel = 4
# chrome_url = "http://localhost:9222"

[diff]
# threshold = 0.0                    # 0.0 = exact match
```

**Override precedence** (highest to lowest):

1. CLI flags (`--url`, `--threshold`, `--parallel`, etc.)
2. Environment variables (`SNAPVRT_STORYBOOK_URL`, `SNAPVRT_DIFF_THRESHOLD`)
3. Config file
4. Defaults

## CLI Commands

```
snapvrt init [--url URL] [--force]        Create .snapvrt/ with config template
snapvrt test [OPTIONS]                     Discover → capture → compare → report
snapvrt update [OPTIONS]                   Discover → capture → save as reference
snapvrt approve [--filter P] [--new] [--failed] [--all]
snapvrt review [--open]                    Generate static HTML report
snapvrt prune [--dry-run] [--yes]          Delete orphaned references
```

**Shared options (test + update):**

- `--url URL` — override Storybook URL
- `--filter PATTERN` / `-f` — case-insensitive name filter
- `--timings` — per-snapshot timing breakdown
- `--parallel N` / `-p` — concurrent browser tabs
- `--screenshot stable|single` — screenshot strategy
- `--chrome-url URL` — connect to remote Chrome

**Test-only options:**

- `--threshold 0.01` — max allowed diff score
- `--prune` — delete orphaned references during test

**Exit codes:** 0 (all pass), 1 (any fail/new/error or config error)

## Storybook Integration

1. Fetch `{url}/index.json`
2. Filter entries: `type == "story"`, exclude `snapvrt-skip` tag
3. Build iframe URL: `{url}/iframe.html?id={story_id}`
4. Build job matrix: stories × viewports

**Docker localhost rewriting:** When `chrome_url` points to a container, `localhost`/`127.0.0.1` in story URLs is rewritten to the host's LAN IP so the containerized Chrome can reach the Storybook dev server.

## Dependencies

| Crate                        | Version | Purpose                               |
| ---------------------------- | ------- | ------------------------------------- |
| tokio                        | 1.49    | Async runtime                         |
| tokio-tungstenite            | 0.28    | CDP WebSocket transport               |
| clap                         | 4.5     | CLI argument parsing                  |
| serde + serde_json           | 1.0     | Serialization                         |
| reqwest                      | 0.13    | HTTP client (Storybook index.json)    |
| image                        | 0.25    | PNG encode/decode                     |
| dify                         | 0.8     | Perceptual image diff                 |
| toml                         | 0.8     | Config parsing                        |
| anyhow + thiserror           | —       | Error handling                        |
| tracing + tracing-subscriber | —       | Structured logging (`RUST_LOG=debug`) |
| base64                       | 0.22    | CDP screenshot data encoding          |
| futures                      | 0.3     | Stream utilities                      |

## Docker Integration (Planned)

**Not yet implemented.** Design from `dev/archive/010-docker-first.md`:

- Image: `ghcr.io/snapvrt/chrome` — Debian slim + Chromium + pinned fonts (Liberation, Noto CJK, Noto Emoji)
- Container management via bollard crate (Docker-compatible API)
- Quick mode: `snapvrt test` auto-starts container, auto-stops after run
- Long-running mode: `snapvrt docker start` keeps container alive across runs
- `--local` flag: skip Docker, use whatever Chrome is installed
- Labels (`snapvrt.managed=true`, `snapvrt.session=<uuid>`) for orphan cleanup

## Future Work

See [ROADMAP.md](ROADMAP.md) for phased plan. Key items:

- **Docker integration** — next priority, closes the gap between "works locally" and "works for users"
- **PDF support** — in-process rendering via pdfium-render
- **Service mode (v1.1)** — HTTP API, `@snapvrt/client`, Jest/Vitest matchers
- **npm distribution** — platform-specific binary packages for `npx snapvrt`
