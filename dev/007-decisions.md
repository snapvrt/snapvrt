# Architecture Decisions

Status: Complete (17/17 resolved) | Date: 2026-02-08

> Part of [snapvrt specification](README.md)

This document records architecture decisions and their rationale.

---

## Resolved

### WorkerPool: Backend-agnostic worker abstraction

**Resolves:** #1 (diff bottleneck), #5 (retry/resilience), #6 (crash cleanup), #7 (protocol versioning), #12 (port management), #14 (load distribution)

The infrastructure concerns are identical for any pool of HTTP workers — whether they're Docker containers, Lambda functions, or remote servers:

- Manage N worker endpoints
- Health check them (with protocol version)
- Distribute requests (with load awareness)
- Retry on failure with backoff
- Graceful shutdown

A `WorkerPool` trait in `pool.rs` abstracts this. The orchestrator doesn't know or care what's behind the HTTP endpoints:

```rust
trait WorkerPool: Send + Sync {
    async fn start(&mut self) -> Result<()>;
    async fn send<Req, Res>(&self, req: Req) -> Result<Res>;
    async fn shutdown(&self) -> Result<()>;
}
```

`DockerPool` is the v1 implementation, handling Docker-specific concerns:

```rust
struct DockerPool {
    image: String,
    count: usize,
    clients: Vec<HttpClient>,
    idle_timeout: Duration,
    retry_policy: RetryPolicy,
    labels: HashMap<String, String>,  // snapvrt.managed=true, snapvrt.session=<uuid>
}

impl WorkerPool for DockerPool { ... }
```

Docker-specific concerns (container lifecycle, orphan cleanup via labels, OS-assigned ports) live in `DockerPool`, not in the trait.

`StaticPool` is the dev/local mode — connects to already-running processes at known URLs. No container management, no startup latency:

```rust
struct StaticPool {
    urls: Vec<Url>,
}

impl WorkerPool for StaticPool { ... }
```

Usage — the orchestrator is backend-agnostic:

```rust
// Production: containers managed by DockerPool
let shots: Box<dyn WorkerPool> = Box::new(
    DockerPool::new("ghcr.io/snapvrt/shot:0.1.0", count: 4)
);

// Dev: local processes at static URLs
let shots: Box<dyn WorkerPool> = Box::new(
    StaticPool::new(vec!["http://localhost:7281"])
);
```

All workers speak HTTP. Same protocol regardless of backend. This is why we chose HTTP over volume mounts for the diff service — it keeps the protocol universal.

**Open questions (discuss later):**

1. **Typed HTTP clients.** Should we wrap the worker protocols in typed Rust client structs (`ShotClient`, `SpotClient`) that encode the protocol, rather than having callers construct raw HTTP requests?

2. **Pool composition: `pool.send()` vs `pool.acquire()`.** Should the pool dispatch requests directly, or hand out a connection/client to a specific worker that the caller uses and returns?

Health check includes protocol version (see [004-protocols.md](004-protocols.md#health-check)). CLI checks compatibility on pool startup and errors if mismatched.

**Impact on architecture:**

- `pool.rs` contains `WorkerPool` trait + `DockerPool` impl + `StaticPool` impl
- Orchestrator uses two pool instances (shot pool + spot pool)
- Both services become independently scalable
- `StaticPool` enables local dev without containers (`--shot-url`, `--spot-url`)
- Future backends (Lambda) slot in without changing orchestration logic

---

### HTTP-native diff protocol

**Resolves:** #2 (protocol mismatch)

Use HTTP idiomatically: multipart for sending images, headers for metadata, body for the result image. Same pattern as the shot service.

**Volume mounts considered and rejected.** Mounting `.snapvrt/snapshots/` into the spot container would skip HTTP entirely, but:

- Locks the protocol to Docker (can't use Lambda, remote servers, etc.)
- Breaks the backend-agnostic abstraction
- Speed gain is marginal: compute (~10ms) dominates transport (~2-3ms)

See [004-protocols.md](004-protocols.md#spot-protocol) for the full protocol specification.

---

### 2-phase diff pipeline with SIMD memcmp

**Resolves:** Original 3-phase pipeline replaced with simpler 2-phase.

The 3-phase pipeline included in-process pixel decoding (Phase 2). This is slow (~15-20ms per image to decode) and memory-heavy (~8-34MB per comparison for raw RGBA buffers). With deterministic containers, re-encoding differences are rare.

See [001-architecture.md](001-architecture.md#2-phase-diff-pipeline) for the pipeline diagram and performance analysis.

**Impact on architecture:**

- `compare.rs` in the snapvrt crate handles the 2-phase pipeline
- No in-process pixel decoding — all pixel work delegated to snapvrt-spot
- `snapvrt-wire` stays clean: types and protocol definitions only. No `image` crate dependency.

---

### God module split: orchestrator + sources

**Resolves:** #3 (engine.rs is a god module)

The old `engine.rs` coordinated story discovery, worker distribution, diffing, result collection, and reporting. As sources grow, this would accumulate all orchestration logic for every source type.

**Resolution:** Split into `orchestrator.rs` + `sources/*`:

```rust
// sources/mod.rs
trait Source: Send + Sync {
    async fn discover(&self) -> Result<Vec<Snapshot>>;
    async fn capture(&self, snapshots: &[Snapshot]) -> Result<Vec<CaptureResult>>;
}
```

The orchestrator only handles the shared compare/report/approve phases. Each source owns its own discovery and capture logic.

---

### Core crate cleanup

**Resolves:** #4 (core crate has misplaced logic)

The old `core` crate had `diff.rs` that didn't belong. With the 2-phase pipeline, all pixel work is delegated to snapvrt-spot. The crate is renamed to `snapvrt-wire` and contains only:

- `types.rs` — `Viewport`, `Png(Vec<u8>)`, `CompareResult`, `DiffResult`
- `protocol.rs` — Header constants, health response type

Dependencies: `serde` only.

---

### PDF rendering with PDFium

**Resolves:** #8 (merge default, PDF engine selection)

**PDF rendering engine:** `pdfium-render` crate wrapping Google's PDFium (BSD-3 license). Same engine Chrome uses for its built-in PDF viewer. Fastest MIT-compatible option (~5-15ms/page at 150 DPI).

| Option     | License    | Speed/page | Verdict       |
| ---------- | ---------- | ---------- | ------------- |
| **PDFium** | BSD-3      | ~5-15ms    | **Selected**  |
| MuPDF      | AGPL-3.0   | ~3-10ms    | Can't bundle  |
| Poppler    | GPL-2.0    | ~30-70ms   | Can't bundle  |
| Chrome CDP | Apache-2.0 | ~200-500ms | Fallback only |

snapvrt-shot integration — PDF rendering is in-process alongside Chrome, no extra container:

```
snapvrt-shot container image:
  Chromium + CDP (see poc/cdp-raw)  → POST /screenshot/web
  libpdfium.so (~25MB) + pdfium-render  → POST /screenshot/pdf
```

**Default `merge: false`:** PDFium renders per-page natively. Per-page is the natural unit — targeted diffs, smaller images, page-level change tracking. Merge (vertical stitch) is opt-in.

**PDF metadata (page count, dimensions) extracted in the CLI** using `lopdf` (MIT) — no worker round-trip needed.

---

### Report uses file references

**Resolves:** #9 (report HTML won't scale)

Report uses relative file references to images on disk. Scales to any project size.

```html
<img src="./snapshots/storybook/button-primary-desktop/reference.png" />
```

Future `--self-contained` flag inlines images for a single portable file.

---

### PDF buffer → URL flow

**Resolves:** #10

Jest sends PDF buffer → service writes to temp file → serves via `/files/*` endpoint → snapvrt-shot fetches URL → renders → service cleans up temp file after comparison.

---

### One unified API

**Resolves:** #11 (Review UI and Service API are duplicated)

One service API is the single source of truth. The review UI is a static SPA served from `/review` that makes fetch calls to the same endpoints (`/status`, `/approve`). No `/api/*` namespace, no duplicate endpoints. See [005-cli-design.md](005-cli-design.md#service-api) for the endpoint reference.

---

### Store locking deferred

**Resolves:** #13

No locking for v1. Concurrent batch operations rejected (409), approve is idempotent (atomic rename), and concurrent approves of different snapshots don't conflict. Revisit if a real race condition surfaces.

---

### Png newtype

**Resolves:** #15

Define `pub struct Png(pub Vec<u8>)` in `snapvrt-wire/types.rs`. Prevents accidentally passing arbitrary bytes where PNG data is expected.

---

### Engine-specific score semantics

**Resolves:** #16

No normalization — different engines have fundamentally different scales. See [004-protocols.md](004-protocols.md#score-semantics) for details.

---

### seccomp=unconfined deferred

**Resolves:** #17

Ship with `unconfined` for v1. snapvrt-shot only navigates to user-controlled URLs (their own Storybook, their own PDFs). Research a custom Chrome seccomp profile during post-MVP hardening.

**Risk note:** `seccomp=unconfined` + `--no-sandbox` means Chrome runs with zero sandboxing. A malicious Storybook story could escape the container. For shared CI environments this matters. Prioritize a custom seccomp profile post-MVP.
