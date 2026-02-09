# Rust Architecture

Status: Accepted | Date: 2026-02-08

> Part of [snapvrt specification](README.md)

This document defines the Rust workspace structure, crate boundaries, and module architecture.

## Workspace Structure

> See [002-project-structure.md](002-project-structure.md) for the full monorepo layout.

```
rust/
├── Cargo.toml                # Workspace manifest
├── Cargo.lock
├── rust-toolchain.toml
└── crates/
    ├── snapvrt/              # CLI binary (published to crates.io)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── main.rs
    │       ├── cli.rs           # Clap definitions, arg parsing, command dispatch
    │       ├── config.rs        # Configuration loading
    │       ├── orchestrator.rs  # Top-level: discover → capture → compare → report
    │       ├── pool.rs          # WorkerPool trait + DockerPool (bollard) + StaticPool (dev)
    │       ├── compare.rs       # 2-phase diff pipeline: memcmp → delegate to spot pool
    │       ├── store.rs         # Snapshot filesystem operations (.snapvrt/)
    │       ├── reporter.rs      # Terminal output (progress, summary) + HTML report
    │       ├── server.rs        # HTTP API service (axum). Thin layer over orchestrator
    │       ├── review.rs        # Serve review UI, approve actions
    │       ├── sources/
    │       │   ├── mod.rs       # Source trait definition
    │       │   ├── storybook.rs # Fetch index.json, filter stories, build URLs
    │       │   └── pdf.rs       # Parse manifest, extract metadata (lopdf)
    │       └── clients/
    │           ├── mod.rs
    │           ├── shot.rs      # Typed HTTP client for snapvrt-shot (reqwest)
    │           └── spot.rs      # Typed HTTP client for snapvrt-spot (reqwest)
    │
    ├── snapvrt-wire/            # Shared types & wire contract (internal)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── types.rs         # Viewport, Png(Vec<u8>), CompareResult, DiffResult
    │       └── protocol.rs      # Header constants, health response type
    │
    ├── snapvrt-shot/            # Screenshot + PDF worker binary (container)
    │   ├── Cargo.toml
    │   └── src/
    │       ├── main.rs
    │       ├── server.rs        # Axum endpoints (/screenshot/web, /screenshot/pdf, /health)
    │       ├── browser.rs       # Chrome CDP connection (see poc/cdp-raw)
    │       ├── tab_pool.rs      # Concurrent browser tab management
    │       ├── screenshot.rs    # Navigate, inject styles, wait for ready, capture
    │       └── pdf.rs           # PDF → PNG rendering (pdfium-render)
    │
    └── snapvrt-spot/            # Diff service binary (container)
        ├── Cargo.toml
        └── src/
            ├── main.rs
            ├── server.rs        # Axum endpoint (POST /diff, /health)
            ├── engine.rs        # Pluggable comparison engine dispatch
            └── engines/         # Implementations (MIT-compatible only)
                ├── mod.rs
                ├── dify.rs
                ├── odiff.rs
                └── imagemagick.rs
```

## Crates

| Crate          | Type    | Published | Description                                             |
| -------------- | ------- | --------- | ------------------------------------------------------- |
| `snapvrt`      | Binary  | crates.io | Main CLI, HTTP server, orchestration                    |
| `snapvrt-wire` | Library | Internal  | Shared types, wire contract between CLI ↔ shot ↔ spot |
| `snapvrt-shot` | Binary  | Container | Screenshot + PDF service (Chrome, PDFium)               |
| `snapvrt-spot` | Binary  | Container | Diff service (image comparison)                         |

### Why These Names

| Crate          | Rationale                                                                                               |
| -------------- | ------------------------------------------------------------------------------------------------------- |
| `snapvrt`      | User-facing tool. `cargo install snapvrt` → `snapvrt` binary.                                           |
| `snapvrt-wire` | The wire layer: types + protocol contract between all services. Short, thematic.                        |
| `snapvrt-shot` | Takes shots (screenshots, PDF pages). Echoes "screenshot" and "snapshot." No ambiguity with WorkerPool. |
| `snapvrt-spot` | Spots differences. "Spot the difference" — the classic visual puzzle.                                   |

### Workspace Cargo.toml

```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/snapvrt/snapvrt"

[workspace.dependencies]
tokio = { version = "1.49", features = ["full"] }
axum = "0.8"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
tracing = "0.1"
```

### Dependency Graph

```
snapvrt ──────────► snapvrt-wire ◄──────────── snapvrt-shot
    (clap, bollard,     (serde)         (axum, see poc/cdp-raw,
     reqwest, axum,                      pdfium-render)
     lopdf)                    ▲
                               │
                          snapvrt-spot
                        (axum, dify)
```

Three binaries communicate via HTTP only. No direct Rust dependencies between `snapvrt`, `snapvrt-shot`, and `snapvrt-spot`. All three depend on `snapvrt-wire` for shared types.

### Container Images

| Crate          | Image                                                                                               |
| -------------- | --------------------------------------------------------------------------------------------------- |
| `snapvrt-shot` | `ghcr.io/snapvrt/shot`                                                                              |
| `snapvrt-spot` | `ghcr.io/snapvrt/spot-dify`, `ghcr.io/snapvrt/spot-odiff`, `ghcr.io/snapvrt/spot-imagemagick` |

## snapvrt Crate Modules

### Module Responsibilities

| Module              | Responsibility                                                   |
| ------------------- | ---------------------------------------------------------------- |
| `cli`               | Clap definitions, arg parsing, command dispatch                  |
| `config`            | Load TOML, env vars, CLI args. Merge with precedence             |
| `orchestrator`      | Top-level: discover → capture → compare → report                 |
| `pool`              | `WorkerPool` trait + `DockerPool` (bollard) + `StaticPool` (dev) |
| `compare`           | 2-phase diff pipeline: memcmp → delegate to spot pool            |
| `store`             | Snapshot filesystem ops (`.snapvrt/`). Read/write snapshots      |
| `reporter`          | Terminal output (progress, summary) and HTML report generation   |
| `server`            | HTTP API service (axum). Thin layer over orchestrator            |
| `review`            | Serve review UI, handle approve actions                          |
| `sources/mod`       | `Source` trait definition                                        |
| `sources/storybook` | Fetch index.json from Storybook, filter stories, build URLs      |
| `sources/pdf`       | Parse manifest, extract metadata (lopdf)                         |
| `clients/shot`      | Typed HTTP client for snapvrt-shot (reqwest)                     |
| `clients/spot`      | Typed HTTP client for snapvrt-spot (reqwest)                     |

### Module Dependencies

```
                         ┌─────┐
                         │ cli │
                         └──┬──┘
                ┌───────────┼───────────┐
                ▼           ▼           ▼
           ┌────────┐  ┌────────┐  ┌────────┐
           │ config │  │ server │  │ review │
           └────────┘  └───┬────┘  └───┬────┘
                           │           │
                           └─────┬─────┘
                                 ▼
                          ┌──────────────┐
                          │ orchestrator │
                          └──────┬───────┘
              ┌────────┬─────────┼─────────┬──────────┐
              ▼        ▼         ▼         ▼          ▼
        ┌──────────┐ ┌──────┐ ┌───────┐ ┌─────────┐ ┌──────────┐
        │ sources/ │ │ pool │ │ store │ │ compare │ │ reporter │
        └──────────┘ └──────┘ └───────┘ └────┬────┘ └──────────┘
                                             │
                                        ┌────┴────┐
                                        │clients/ │
                                        └─────────┘
```

| From           | To                                      |
| -------------- | --------------------------------------- |
| `cli`          | config, server, orchestrator, review    |
| `server`       | orchestrator                            |
| `review`       | orchestrator, store                     |
| `orchestrator` | sources, pool, store, compare, reporter |
| `compare`      | clients/spot                            |

### Error Types

The spec defines exit codes (0, 1, 2) and `thiserror` is a workspace dependency, but no error type hierarchy is designed yet. With multiple sources, worker types, and failure modes (config error, Docker not running, worker timeout, network failure, diff engine error), a well-designed error enum matters. Define during Phase 2 implementation.

### Key Types

```rust
// config.rs
pub struct Config {
    pub port: u16,
    pub host: IpAddr,
    pub snapshot_dir: PathBuf,
    pub sources: SourcesConfig,
    pub service: ServiceConfig,
}

// orchestrator.rs
pub struct Orchestrator {
    config: Arc<Config>,
    shot_pool: Box<dyn WorkerPool>,
    spot_pool: Box<dyn WorkerPool>,
    store: Store,
}

impl Orchestrator {
    pub async fn test_storybook(&self, filter: Option<&str>) -> Result<TestResult>;
    pub async fn update_storybook(&self, filter: Option<&str>) -> Result<UpdateResult>;
    pub async fn compare_pdf(&self, req: ComparePdfRequest) -> Result<CompareResult>;
    pub async fn compare_web(&self, req: CompareWebRequest) -> Result<CompareResult>;
    pub async fn approve(&self, name: &str) -> Result<()>;
    pub async fn approve_all(&self) -> Result<()>;
    pub async fn status(&self) -> Result<Status>;
}

// pool.rs — see 007-decisions.md for WorkerPool design rationale
trait WorkerPool: Send + Sync {
    async fn start(&mut self) -> Result<()>;
    async fn send<Req, Res>(&self, req: Req) -> Result<Res>;
    async fn shutdown(&self) -> Result<()>;
}

// sources/mod.rs
trait Source: Send + Sync {
    async fn discover(&self) -> Result<Vec<Snapshot>>;
    async fn capture(&self, snapshots: &[Snapshot]) -> Result<Vec<CaptureResult>>;
}

// sources/storybook.rs
pub struct StorybookSource {
    url: Url,
}

impl Source for StorybookSource { /* ... */ }

// store.rs
pub struct Store {
    root: PathBuf,  // .snapvrt/
}

impl Store {
    pub fn reference_path(&self, source: &str, name: &str) -> PathBuf;
    pub fn current_path(&self, source: &str, name: &str) -> PathBuf;
    pub fn diff_path(&self, source: &str, name: &str) -> PathBuf;
    pub fn list_pending(&self, source: Option<&str>) -> Result<Vec<PendingDiff>>;
    pub fn approve(&self, source: &str, name: &str) -> Result<()>;
}
```

## snapvrt-wire Crate

Shared types and wire contract used by `snapvrt`, `snapvrt-shot`, and `snapvrt-spot`.

Dependencies: `serde`

```rust
// types.rs
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub device_scale_factor: f32,
}

pub struct Png(pub Vec<u8>);

pub struct CompareResult {
    pub match_: bool,
    pub score: f64,
    pub is_new: bool,
}

pub struct DiffResult {
    pub is_match: bool,
    pub score: f64,
    pub diff_png: Option<Vec<u8>>,
}
```

Protocol constants and health response type are defined here. See [004-protocols.md](004-protocols.md) for the full wire protocol specification.

## snapvrt-shot Crate

Runs inside containers. Receives URLs, returns PNGs.

### Module Responsibilities

| Module       | Responsibility                                                        |
| ------------ | --------------------------------------------------------------------- |
| `server`     | Axum HTTP endpoints (`/screenshot/web`, `/screenshot/pdf`, `/health`) |
| `browser`    | Chrome CDP connection (see poc/cdp-raw)                               |
| `tab_pool`   | Manages N concurrent browser tabs                                     |
| `screenshot` | Navigate, wait for ready, capture                                     |
| `pdf`        | PDF rendering to PNG (pdfium-render)                                  |

### Tab Pool

Worker manages a pool of browser tabs for concurrent captures:

```rust
pub struct TabPool {
    tabs: Vec<Tab>,
    semaphore: Semaphore,
}

impl TabPool {
    pub fn new(size: usize) -> Self;
    pub async fn capture(&self, req: WebScreenshotRequest) -> Result<Png>;
}
```

Tab count from environment: `SNAPVRT_TABS=4`

See [004-protocols.md](004-protocols.md) for the shot HTTP protocol.

## snapvrt-spot Crate

Runs inside containers. Receives two PNGs, returns match/score/diff.

### Module Responsibilities

| Module     | Responsibility                                   |
| ---------- | ------------------------------------------------ |
| `server`   | Axum HTTP endpoint (`POST /diff`, `/health`)     |
| `engine`   | Pluggable comparison engine dispatch      |
| `engines/` | Implementations (dify, odiff, imagemagick) |

### Pluggable Engines

Each engine implements a common trait:

```rust
pub trait DiffEngine: Send + Sync {
    fn compare(&self, reference: &[u8], current: &[u8]) -> Result<(f64, Option<Vec<u8>>)>;
}
```

**Bundled engines (MIT-compatible):**

| Engine        | License    | Notes                                       |
| ------------- | ---------- | ------------------------------------------- |
| `dify`        | MIT        | Default, YIQ perceptual + anti-aliasing     |
| `odiff`       | MIT        | SIMD-optimized, very fast                   |
| `imagemagick` | Apache-2.0 | Multiple algorithms                         |

Engine selected via environment: `SNAPVRT_DIFF_ENGINE=dify`

**External engines:** Users can configure any container image that implements the `POST /diff` protocol (see [004-protocols.md](004-protocols.md#spot-protocol)).

See [004-protocols.md](004-protocols.md) for the spot HTTP protocol.

## Runtime Architecture

### CLI Command Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ snapvrt <command>                                                           │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
                              ┌─────────────────┐
                              │  config::load() │
                              └────────┬────────┘
                                       │
                                       ▼
                         ┌─────────────────────────────┐
                         │ command = "service start"?  │
                         └─────────────┬───────────────┘
                                       │
                      ┌────────────────┴────────────────┐
                      │ yes                             │ no
                      ▼                                 ▼
          ┌───────────────────────┐        ┌───────────────────────┐
          │ server::start(orch)   │        │ GET :{port}/health    │
          │                       │        └───────────┬───────────┘
          │ (blocks, serves HTTP) │                    │
          └───────────────────────┘       ┌────────────┴────────────┐
                                          │ 200 OK                  │ refused
                                          ▼                         ▼
                              ┌─────────────────────┐  ┌─────────────────────────┐
                              │ delegate to service │  │ orchestrator::run(cmd)  │
                              │                     │  │                         │
                              │ POST :{port}/...    │  │ (standalone mode)       │
                              └─────────────────────┘  └─────────────────────────┘
```

**Paths:**

- `service start` → start HTTP server, block
- batch + service running → delegate via HTTP
- batch + no service → run orchestrator directly (standalone)

### Service Mode

```
┌────────────────────────────────────────────────────────────────────────────┐
│ SERVICE MODE (snapvrt service start)                                       │
│                                                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │ HTTP API (:{port}, default 7280)                                     │  │
│  │                                                                      │  │
│  │  GET  /health ─────────── returns 200 (enables CLI detection)        │  │
│  │  GET  /status ─────────── pending diffs, running operations          │  │
│  │  POST /storybook/test ─── run Storybook test (409 if busy)           │  │
│  │  POST /pdf/compare ────── compare PDF                                │  │
│  │  POST /web/compare ────── compare web page screenshot                │  │
│  │  POST /approve ────────── approve single snapshot                    │  │
│  │  GET  /review ─────────── serve review UI                            │  │
│  │  WS   /ws ─────────────── live updates                               │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                   │                                        │
│                                   ▼                                        │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │ Container Manager                                                    │  │
│  │                                                                      │  │
│  │  ┌─────────────────────┐  ┌─────────────────────┐                    │  │
│  │  │ shot (Chrome + PDF) │  │ spot (dify)         │                    │  │
│  │  │ POST /screenshot/*  │  │ POST /diff          │                    │  │
│  │  └─────────────────────┘  └─────────────────────┘                    │  │
│  │                                                                      │  │
│  │  Both: 5-min idle timeout, auto-stop, restart on request             │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────────┘
```

See [005-cli-design.md](005-cli-design.md#service-api) for full service API endpoint reference.

## Service Mode (Deferred to v1.1)

Q1-Q6 (service detection, container lifecycle, concurrent operations, binding, review mode, daemon mode) are deferred. v1 is batch CLI only. Service mode, Jest/Vitest integration, and the `@snapvrt/client` package come in v1.1.

The service mode architecture (runtime diagrams above) remains the design target for v1.1. The leanings recorded in [007-decisions.md](007-decisions.md) stand as starting points when implementation begins.

## Licensing

All code in the main snapvrt repository is MIT licensed.

Only MIT-compatible diff engines are bundled (dify/MIT, odiff/MIT, imagemagick/Apache-2.0). Third-party engines (like dssim/AGPL-3.0) are maintained in separate repositories and configured via:

```toml
[diff]
image = "ghcr.io/snapvrt/spot-dssim"  # AGPL-3.0, separate repo
```
