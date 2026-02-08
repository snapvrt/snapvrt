# System Architecture

Status: Accepted | Date: 2026-02-08

> Part of [snapvrt specification](README.md)

This document defines the system architecture: component diagram, separation of concerns, and design rationale.

## Architecture

```
┌───────────────────────────────────────────────────────────────────────────┐
│ Host Machine                                                              │
│                                                                           │
│  ┌──────────────────────────────────────────────────────────────────────┐ │
│  │ snapvrt CLI (orchestrator)                                           │ │
│  │                                                                      │ │
│  │  1. Discover snapshots (source-specific)                             │ │
│  │  2. Capture screenshots via shot pool                                │ │
│  │  3. Compare: memcmp → delegate mismatches to spot pool               │ │
│  │  4. Report results                                                   │ │
│  └─────────────────┬─────────────────────────────────────┬──────────────┘ │
│                    │ HTTP                                │ HTTP           │
│           ┌────────┴──────────┐                 ┌────────┴─────────┐      │
│           │                   │                 │                  │      │
│           ▼                   ▼                 ▼                  ▼      │
│  ┌─────────────────┐ ┌─────────────────┐ ┌─────────────┐ ┌─────────────┐  │
│  │ shot 1          │ │ shot N          │ │ spot 1      │ │ spot M      │  │
│  │ (container)     │ │ (container)     │ │ (container) │ │ (container) │  │
│  │                 │ │                 │ │             │ │             │  │
│  │ Chrome + PDFium │ │ Chrome + PDFium │ │ POST /diff  │ │ POST /diff  │  │
│  │ ├─Tab 1 ─► PNG  │ │ ├─Tab 1 ─► PNG  │ │ PNG ─► Score│ │ PNG ─► Score│  │
│  │ ├─Tab 2 ─► PNG  │ │ ├─Tab 2 ─► PNG  │ └─────────────┘ └─────────────┘  │
│  │ └─Tab K ─► PNG  │ │ └─Tab K ─► PNG  │                                  │
│  └─────────────────┘ └─────────────────┘                                  │
│                                                                           │
│  .snapvrt/                                                                │
│  ├── snapshots/                                                           │
│  │   ├── reference.png  ◄── committed baselines                           │
│  │   ├── current.png    ◄── CLI writes after capture                      │
│  │   └── diff.png       ◄── CLI writes after comparison                   │
│  └── report.html                                                          │
└───────────────────────────────────────────────────────────────────────────┘

Alternative shot backends (same protocol):
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│ AWS Lambda       │    │ Browserstack     │    │ Local Chrome     │
└──────────────────┘    └──────────────────┘    └──────────────────┘

Alternative spot engines (same protocol):
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│ pixelmatch       │    │ odiff            │    │ imagemagick      │
└──────────────────┘    └──────────────────┘    └──────────────────┘
```

## Separation of Concerns

| Component              | Responsibility                                            |
| ---------------------- | --------------------------------------------------------- |
| **CLI (orchestrator)** | Source discovery, task distribution, compare, report      |
| **shot pool**          | Receive URL → return PNG (web via Chrome, PDF via PDFium) |
| **spot pool**          | Receive two PNGs → return match/score/diff                |

## Why This Design

- **Containers** - Consistent screenshots AND diffs across all platforms
- **Parallelization** - N shots x K tabs for capture, M spots for comparison — scaled independently
- **Pluggable** - HTTP protocol enables alternative backends (see [protocols](004-protocols.md))
- **CLI has context** - Access to git, filesystem, config
- **Services are stateless** - URLs in, PNGs out; PNGs in, scores out

## 2-Phase Diff Pipeline

Minimizes diff service calls by doing a fast local check first:

```
┌─────────────────────────────────────────────────────────────┐
│ Phase 1: File Compare (SIMD memcmp, ~0.02ms per snapshot)   │
│ reference bytes == current bytes?                           │
│ ├── Match → DONE (unchanged)                                │
│ └── Differ → Phase 2                                        │
├─────────────────────────────────────────────────────────────┤
│ Phase 2: Full Diff (diff service, ~50ms)                    │
│ Run diff engine, generate score + diff image                │
│ └── Return { score, diff.png }                              │
└─────────────────────────────────────────────────────────────┘
```

**Why memcmp over hashing:** Dead simple, no metadata files to maintain. Fast enough at all realistic scales (~1s for 50K snapshots with 8-core parallelism). Containers produce deterministic PNGs, so re-encoding differences are rare — the diff service handles those in Phase 2.

| Scenario                | Without optimization   | With optimization |
| ----------------------- | ---------------------- | ----------------- |
| 100 unchanged           | 100 diff service calls | 0 calls           |
| 95 unchanged, 5 changed | 100 diff service calls | 5 calls           |

## Future Backends

| Backend    | Shot   | Spot | Use Case            |
| ---------- | ------ | ---- | ------------------- |
| Docker     | v1     | v1   | Default, consistent |
| Static     | v1     | v1   | Dev/local mode      |
| AWS Lambda | Future | -    | Scale to thousands  |

See [003-rust-crates.md](003-rust-crates.md) for `WorkerPool` trait and backend implementations.

## Open Concerns

### Observability

Worker protocols include `X-Timing-*` headers, but there's no structured observability:

- No tracing spans across CLI → worker requests (tracing is a workspace dependency but not integrated)
- No metrics collection (screenshot duration distribution, diff rate, container startup time)
- No error correlation (which story caused which worker to fail)

For a CI tool, debugging "why did visual tests fail" requires good observability. Address during Phase 2 (CLI Core).

### Performance Targets

The spec says "fast" but defines no concrete targets:

- Time budget for `snapvrt test` at 100 / 500 / 1000 stories?
- Acceptable container startup latency?
- Memory budget per worker?

Define targets after Phase 0 PoCs provide real measurements.
