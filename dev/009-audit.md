# 009 — Codebase Audit: Docs vs Implementation

Date: 2026-02-12

This document is a comprehensive audit of the snapvrt project, comparing what the design docs describe versus what is actually implemented. It identifies conflicts, gaps, stale documentation, and proposes a path forward.

## TL;DR

The project has a solid working CLI (`init`, `test`, `update`, `approve`, `review`, `prune`) that can capture screenshots from Storybook via CDP, diff them with dify, and report results. The core loop works. However, the documentation has diverged significantly from reality due to a mid-project architecture pivot (010-docker-first.md), and large parts of the spec describe features that don't exist yet. The most immediate blocker is Docker integration — the tool currently requires a manually-running Chrome instance.

---

## 1. The Architecture Pivot

The single most important thing to understand about this codebase is that **010-docker-first.md supersedes 001-architecture.md and 003-rust-crates.md**, but the old docs still exist and are still linked from the spec index (dev/README.md) without any deprecation notice.

### Original Architecture (001/003)

- 4 Rust crates: `snapvrt`, `snapvrt-wire`, `snapvrt-capture`, `snapvrt-diff`
- Capture runs in a container (`ghcr.io/snapvrt/capture`) as an HTTP service
- Diff runs in a container (`ghcr.io/snapvrt/diff-*`) as an HTTP service
- CLI talks to both over HTTP
- `WorkerPool` trait abstracts container management
- Wire protocols (004) define the HTTP contracts between components

### Current Architecture (010)

- 1 Rust crate: `snapvrt`
- Capture is in-process — CLI connects directly to Chrome via CDP WebSocket
- Diff is in-process — dify runs on `spawn_blocking`
- Only Chrome runs in a container (`ghcr.io/snapvrt/chrome`)
- No HTTP protocol between CLI and capture/diff
- No `WorkerPool` trait needed for v1

### What's Actually Implemented

- **Matches 010.** Single crate, in-process capture via CDP, in-process dify.
- But Docker integration is **not implemented** — no bollard, no container management.
- Currently requires a pre-running Chrome instance at `--chrome-url` or launches a local Chrome process.

### Conflict Impact: HIGH

Docs 001, 003, and 004 describe an architecture that will never exist in v1. Anyone reading the spec linearly (which is the natural thing to do) will form a completely wrong mental model of the system.

---

## 2. Store Layout Conflict

Three different store layouts appear across the documentation and implementation.

### Spec README (lines 71-87) / 005-cli-design.md (lines 296-313)

```
.snapvrt/snapshots/
├── storybook/
│   └── button-primary-desktop/
│       ├── reference.png
│       ├── current.png
│       └── diff.png
```

Per-snapshot directories. Reference, current, and diff images live together.

### 010-docker-first.md (line 333)

```
.snapvrt/{reference,current,difference}/
```

Per-type directories. All references in one dir, all currents in another.

### Implementation (store/mod.rs)

```
.snapvrt/
├── reference/
│   └── {source}/{title}/{name}_{viewport}.png
├── current/
│   └── {source}/{title}/{name}_{viewport}.png
└── difference/
    └── {source}/{title}/{name}_{viewport}.png
```

Per-type directories with nested ID paths. Matches 010's description.

### Conflict Impact: MEDIUM

The README and 005 describe the old layout. The implementation follows 010. The HTML report and approve/prune commands depend on the per-type layout. This is a documentation-only fix but it affects anyone trying to understand the snapshot structure.

---

## 3. Config Format Conflict

### 005-cli-design.md

Uses `[sources.storybook]` (plural) with required fields `url` OR `static_dir`:

```toml
[sources.storybook]
url = "http://localhost:6006"
```

### docs/configuration.md / Implementation

Uses `[source.storybook]` (singular) with `type` field:

```toml
[source.storybook]
type = "storybook"
url = "http://localhost:6006"
viewports = ["laptop"]
```

### Conflict Impact: LOW-MEDIUM

The implementation and user-facing docs agree (`[source.X]`), but 005 disagrees. 005 also describes source auto-detection logic (`[sources.*]`) that doesn't match.

Additionally, the implementation has config fields not in any doc:

- `[capture]` section with `preset`, `animation`, `clip`, `screenshot`, `network_wait`, `stability_attempts`, `stability_delay_ms`
- These are documented in `docs/configuration.md` but not in 005-cli-design.md.

---

## 4. CLI Command Conflicts

### cli-reference.md

Lists `service` as a top-level command:

```
Commands:
  init, test, update, approve, review, prune, service
```

### 010-docker-first.md

Introduces `docker` as a top-level command:

```
snapvrt docker start    # Start long-running Chrome container
snapvrt docker stop     # Stop it
snapvrt docker status   # Show running container info
```

Also introduces `--local` flag on `test` and `update`.

### Implementation

Neither `service` nor `docker` commands exist. Commands implemented:

```
init, test, update, approve, review, prune
```

No `--local` flag. No `--auto-add-new` flag. No `--fail-fast` flag.

Extra flags that exist in implementation but not in cli-reference.md:

- `--timings` (print per-snapshot timing breakdown)
- `--threshold` (override diff threshold from CLI)
- `--parallel` (override parallel tab count)
- `--screenshot` (override screenshot strategy)
- `--chrome-url` (connect to remote Chrome)

### Conflict Impact: MEDIUM

The cli-reference.md is stale. It describes some flags that exist (`--filter`, `--prune`, `--dry-run`, `--yes`) but misses others (`--timings`, `--threshold`), and includes commands that don't exist (`service`).

---

## 5. Container Images Conflict

### README / 001 / 003

```
ghcr.io/snapvrt/capture          # Rust binary + Chrome + PDFium
ghcr.io/snapvrt/diff-dify        # Rust binary + dify
ghcr.io/snapvrt/diff-odiff       # Rust binary + odiff
ghcr.io/snapvrt/diff-imagemagick # Rust binary + imagemagick
```

### 010-docker-first.md

```
ghcr.io/snapvrt/chrome            # Chrome-only image
```

No diff containers. No capture binary. Just Chrome.

### Implementation

No container images exist. No `docker/` directory. No Dockerfiles.

### Conflict Impact: MEDIUM

The README still lists 4 container images. The actual plan (010) calls for 1. Zero are built.

---

## 6. Wire Protocols (004) — Status Unclear

004-protocols.md defines HTTP protocols for:

- `POST /screenshot/web` — capture protocol
- `POST /screenshot/pdf` — PDF capture protocol
- `POST /diff` — diff protocol
- `GET /health` — health check

These protocols are **not used by the current implementation** because capture and diff run in-process. However, 010 states:

> Wire protocols (004) — still the target if capture/diff ever become services (v1.1+)

So 004 is not wrong per se, but it describes a future that may or may not happen. It should be clearly marked as "deferred to v1.1+" rather than appearing as current spec.

### Conflict Impact: LOW

Technically accurate for the long-term vision, but misleading for anyone reading the spec as a guide to the current system.

---

## 7. Multi-Source Support

### Spec (005-cli-design.md)

Describes a full multi-source architecture:

- `[SOURCE]` argument on all commands
- Auto-detection of configured sources
- Source-specific options
- Multi-source grouped output
- PDF manifests, web manifests

### Implementation

Single-source only. `config/resolve.rs` extracts one source:

> "Single source extracted (multi-source is future work)"

No PDF support. No web manifest support. No source argument on commands.

### Conflict Impact: LOW

This is expected — the roadmap clearly defers multi-source to later phases. But 005 reads like current spec, not future spec.

---

## 8. JavaScript Packages / npm Distribution

### Spec (006-js-packages.md, README)

Describes:

- `@snapvrt/client` — HTTP client
- `@snapvrt/jest` — Jest matchers
- `@snapvrt/vitest` — Vitest matchers
- `snapvrt` npm package with platform-specific binary downloads
- `node/` workspace with pnpm

### Implementation

None of this exists. No `node/` directory. No JS packages. No npm distribution.

### Conflict Impact: LOW

Clearly deferred to v1.1 per the roadmap. Not a conflict, just not built yet.

---

## 9. What's Implemented But Undocumented

The implementation has several features not covered by any design doc:

| Feature                                | Location                 | Notes                                  |
| -------------------------------------- | ------------------------ | -------------------------------------- |
| Capture presets (`standard` / `loki`)  | `config/capture.rs`      | Loki-compatible capture mode           |
| 9-stage pipeline with strategy pattern | `capture/pipeline.rs`    | More granular than spec describes      |
| Screenshot stability check             | `capture/strategy.rs`    | Mentioned in 008 but not in main spec  |
| Web Animations API control             | `capture/scripts.rs`     | Mentioned in 008 but not in main spec  |
| `captureBeyondViewport`                | `capture/pipeline.rs`    | Mentioned in 008 as implemented        |
| `--timings` flag                       | `cli.rs`                 | Per-snapshot timing breakdown          |
| `SNAPVRT_LOG` env var                  | `main.rs`                | tracing-subscriber env filter          |
| Docker localhost IP rewriting          | `storybook/discovery.rs` | Rewrites localhost to LAN IP           |
| Dimension mismatch handling            | `compare/diff.rs`        | Pads with magenta for size differences |
| Chrome crash detection                 | `capture/runner.rs`      | After 3 consecutive failures           |

008-screenshot-capture.md documents the capture best practices analysis and marks items 1-3 as "Implemented", but the main spec (README, 004) doesn't reflect these implementations.

---

## 10. Roadmap vs Reality

### Phase 0 (PoCs): COMPLETE

All 4 PoCs validated and findings integrated into the main crate.

### Phase 1 (Foundation): COMPLETE (architecture changed)

The original Phase 1 called for building `snapvrt-wire`, `snapvrt-capture`, `snapvrt-diff` + Docker images. With the 010 pivot, this became "build the in-process capture pipeline + dify integration", which is done.

### Phase 2 (CLI Core): MOSTLY COMPLETE

| Item                                                    | Status                                                                 |
| ------------------------------------------------------- | ---------------------------------------------------------------------- |
| Config (TOML loading, env, CLI merge)                   | Done                                                                   |
| Store (`.snapvrt/` filesystem ops)                      | Done                                                                   |
| Pool / Docker (bollard, container lifecycle)            | **NOT DONE**                                                           |
| Orchestrator (discover -> capture -> compare -> report) | Done                                                                   |
| Sources/storybook (discovery + URL building)            | Done                                                                   |
| Storybook error detection                               | Partial (Chrome crash detection, but no `.sb-show-errordisplay` check) |
| Clients/capture, clients/diff (HTTP clients)            | N/A (in-process now)                                                   |
| Compare (2-phase diff pipeline)                         | Done                                                                   |

### Phase 3 (CLI Commands): MOSTLY COMPLETE

| Command   | Status                               |
| --------- | ------------------------------------ |
| `init`    | Done                                 |
| `test`    | Done                                 |
| `update`  | Done                                 |
| `approve` | Done                                 |
| `prune`   | Done                                 |
| `review`  | Done (generates HTML, `--open` flag) |

### Phase 4 (Output): PARTIALLY COMPLETE

| Item                    | Status                                                         |
| ----------------------- | -------------------------------------------------------------- |
| Terminal reporter       | Done                                                           |
| HTML report             | Done (static, file-referenced images)                          |
| `review` as HTTP server | **NOT DONE** (review generates static HTML, not a live server) |

### Phase 5 (PDF): NOT STARTED

### Phase 6 (Service v1.1): NOT STARTED

### Docker Integration (from 010): NOT STARTED

This is the **critical gap** — the `--local` / Docker-default split from 010 isn't implemented. The tool always runs in "local" mode (direct Chrome launch or `--chrome-url`).

---

## 11. Summary of Conflicts

| #   | Conflict                                                                                           | Severity   | Resolution                                           |
| --- | -------------------------------------------------------------------------------------------------- | ---------- | ---------------------------------------------------- |
| 1   | 001/003 describe 4-crate container architecture; 010/impl use single crate                         | HIGH       | Mark 001/003 as superseded in the spec index         |
| 2   | Store layout differs between README/005 and 010/impl                                               | MEDIUM     | Update README and 005 to match implementation        |
| 3   | Config uses `[source.X]` (impl) vs `[sources.X]` (005)                                             | LOW-MEDIUM | Update 005 to match impl                             |
| 4   | CLI reference missing `--timings`/`--threshold`, lists nonexistent `service`                       | MEDIUM     | Rewrite cli-reference.md from actual `--help` output |
| 5   | README lists 4 container images; 010 plans 1; zero exist                                           | MEDIUM     | Update README to match 010                           |
| 6   | 004 wire protocols not used in current architecture                                                | LOW        | Add "deferred" banner to 004                         |
| 7   | 005 describes multi-source as if current; impl is single-source                                    | LOW        | Add "future" markers to 005                          |
| 8   | 008 features (stability check, animations, captureBeyondViewport) implemented but not in main spec | LOW        | Reference 008 findings from main spec                |
| 9   | `review` described as live HTTP server; impl generates static HTML                                 | LOW        | Align docs to impl                                   |
| 10  | CI integration doc uses `--storybook-dir` flag that doesn't exist                                  | LOW        | Update ci-integration.md                             |

---

## 12. Recommended Path Forward

### Immediate (before any new feature work)

1. **Add Docker integration.** This is the biggest gap between what 010 describes and what exists. Without it, the tool requires users to manually run Chrome. Implementation:
   - Add `bollard` dependency
   - `docker/` module: pull image, start/stop container, health check polling
   - `ghcr.io/snapvrt/chrome` Dockerfile (already designed in 010)
   - Default mode: auto-start Chrome container, auto-stop after run
   - `--local` flag: current behavior (launch local Chrome)

2. **Clean up the docs.** The spec has too many active documents that contradict each other. Proposed changes:
   - Add deprecation notice to 001 and 003 referencing 010
   - Update dev/README.md spec index to mark which docs are current vs superseded
   - Update store layout in README to match implementation
   - Rewrite cli-reference.md from actual `--help` output
   - Add "deferred to v1.1" banners to 004, 006, service-related sections of 005
   - Update ci-integration.md to use actual flags

### Short-term (v1 completeness)

3. **`snapvrt docker start/stop/status` command.** Long-running Chrome container for faster iteration cycles. Designed in 010 but not built.

4. **Storybook error detection.** Check for `.sb-show-errordisplay` after navigation. Referenced in ROADMAP Phase 2 but not implemented.

5. **Test infrastructure.** The ROADMAP calls for unit, integration, e2e, and golden tests per phase. Currently only `compare/diff.rs` has tests. Priority areas:
   - Config parsing (unit)
   - Store operations (unit)
   - Storybook discovery (integration, with mock HTTP)
   - Full `snapvrt test` against storybook-basic (e2e)

6. **npm distribution.** Even without JS packages, distributing the binary via npm (`npx snapvrt`) dramatically lowers the adoption barrier vs `cargo install`. The dify project (referenced in 002) is a good model.

### Medium-term

7. **PDF support** (Phase 5). Designed but not started.

8. **Review as live server.** Currently review generates static HTML. The spec envisions `snapvrt review` starting a local HTTP server with live approve actions.

9. **Service mode** (Phase 6 / v1.1). HTTP API, JS packages, Jest/Vitest integration.

### Things I Would NOT Prioritize

- **Multiple diff engines** (odiff, imagemagick). dify works well. Not worth the complexity.
- **Multi-source in v1.** Get Storybook working perfectly first.
- **`WorkerPool` trait.** The 010 architecture doesn't need it for v1.
- **Lambda/cloud backends.** Way too early.
- **Custom seccomp profile.** Ship with `unconfined` for now, document the risk.

---

## 13. Code Quality Assessment

The implementation is clean and well-structured:

- **~3,500 lines** across 32 source files
- Clear module boundaries (cdp, capture, compare, storybook, store, config, report, commands)
- Good error handling with `anyhow`
- Structured logging via `tracing`
- The capture pipeline (9 stages, strategy pattern, presets) is more sophisticated than the spec describes
- The diff engine has proper test coverage including edge cases

The main risk is not code quality but **the gap between "works on the developer's machine" and "works for users"** — which is exactly what Docker integration closes.
