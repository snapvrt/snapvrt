# Roadmap

## Phase 0: Validate (PoCs)

Minimal prototypes to validate core assumptions before building the full solution. Each PoC is a standalone Rust example or project — throwaway code that proves a capability works.

### 0.1 Storybook 10 example project

Create a minimal Storybook 10 project in `examples/storybook-basic/`. A few simple components with stories. This serves two purposes:

- Validate the `index.json` API format assumed in the spec
- Become the example project for snapvrt users later

### 0.2 PoC: CDP screenshot capture

Standalone Rust binary that:

1. Connects to a running Chrome instance via CDP (chromiumoxide)
2. Navigates to a URL passed as CLI arg
3. Injects animation-disabling CSS
4. Waits for ready (network idle + fonts + DOM stable)
5. Screenshots `<body>` bounding box
6. Saves PNG to disk

**Validates:** chromiumoxide API, ready detection strategy, screenshot cropping.

### 0.3 PoC: Storybook source discovery

Standalone Rust binary that:

1. Fetches `http://localhost:6006/index.json`
2. Parses the response
3. Filters to `type: "story"`, excludes `snapvrt-skip` tag
4. Prints the list of story IDs and URLs

Run against the example project from 0.1.

**Validates:** Storybook 10 index.json format, story filtering logic.

### 0.4 PoC: Image comparison (pixelmatch)

Standalone Rust binary that:

1. Takes two PNG file paths as args
2. Runs pixelmatch comparison
3. Outputs match/score
4. Writes diff PNG if mismatched

**Validates:** pixelmatch Rust bindings (or WASM/FFI approach), diff image generation.

### Outcome

Findings from PoCs feed back into the design docs. Update architecture and protocols if assumptions were wrong.

---

## Phase 1: Foundation

Build the real crates based on validated PoCs.

- `snapvrt-wire` — shared types (`Viewport`, `Png`, `CompareResult`, `DiffResult`, protocol constants)
- `snapvrt-shot` — HTTP server wrapping the CDP screenshot PoC (web only, PDF later)
- `snapvrt-spot` — HTTP server wrapping the pixelmatch PoC
- Docker images for both

**Milestone:** Can `POST /screenshot/web` to a container and get a PNG back. Can `POST /diff` with two PNGs and get a score.

## Phase 2: CLI Core

- `config` — TOML loading, env vars, CLI flag merging
- `store` — `.snapvrt/` filesystem operations (read/write snapshots, atomic writes)
- `pool` — `WorkerPool` trait, `DockerPool` (bollard), `StaticPool`
- `orchestrator` — discover → capture → compare → report pipeline
- `sources/storybook` — story discovery + URL building
- `clients/shot`, `clients/spot` — typed HTTP clients
- `compare` — 2-phase diff pipeline (memcmp → spot)

**Milestone:** `cargo run -- test` works end-to-end against a running Storybook.

## Phase 3: CLI Commands

- `init` — create `.snapvrt/` directory, config, gitignore
- `test` — full test run with exit codes
- `update` — capture references directly
- `approve` — copy current → reference (with filters)
- `prune` — remove orphaned references

**Milestone:** All batch CLI commands work. Can run in CI.

## Phase 4: Output

- `reporter` — terminal output (progress bars, summary, symbols)
- HTML report generation (static, file-referenced images)
- `review` — lightweight HTTP server serving review UI + approve actions

**Milestone:** `snapvrt review` opens browser with side-by-side diffs.

## Phase 5: PDF Support

- `snapvrt-shot` — add `/screenshot/pdf` endpoint (pdfium-render)
- `sources/pdf` — manifest parsing, metadata extraction (lopdf)
- Multi-page snapshot support (per-page directories, manifest.json)
- Page count change detection (synthetic diffs for added/removed pages)

**Milestone:** `snapvrt test pdf` works with a PDF manifest.

## Phase 6: Service Mode (v1.1)

- `server` — HTTP API (axum) wrapping the orchestrator
- Service detection (health check probing)
- Container idle timeout management
- `@snapvrt/client` — JS HTTP client
- `@snapvrt/jest` — Jest matchers
- `@snapvrt/vitest` — Vitest matchers
- Resolve deferred Q1-Q6 from 003-rust-crates.md

**Milestone:** `snapvrt service start` + `npm test` with Jest matchers works.

---

## Cross-Cutting: Test Strategy

Each phase should include tests for the code it introduces:

- **Unit tests** — config parsing, store operations, compare logic, protocol serialization
- **Integration tests** — real containers (shot + spot) with test fixtures
- **End-to-end tests** — full `snapvrt test` against the example Storybook from Phase 0.1
- **Golden tests** — HTML report output against known-good snapshots
- **CI pipeline** — run the above on every PR

Define concrete test coverage expectations as each phase is implemented.
