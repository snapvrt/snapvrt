# snapvrt Specification

Status: Accepted | Date: 2026-02-08

## Overview

Visual regression testing for Storybook, PDFs, and more.

**Tagline:** Snap. Test. Ship.

**Sources:**

- Storybook 10+ (v1)
- PDF via batch CLI (v1), service API (v1.1)
- Web pages, Figma (future)

## Design Principles

1. **Rust + MIT** - Single binary CLI, permissive license
2. **Containers** - Screenshots and diffs run in containers for cross-platform consistency
3. **Storybook 10 only** - No legacy API support
4. **Zero configuration** - Sensible defaults, minimal setup
5. **Fast** - Parallel workers, native tools
6. **Pluggable** - Simple protocols enable alternative backends

## Roadmap

See [ROADMAP.md](ROADMAP.md) — PoC-first approach, then incremental build-up. Service mode deferred to v1.1.

## Architecture

See [001-architecture.md](001-architecture.md) for the system architecture diagram, component separation, 2-phase diff pipeline, and design rationale.

## Design Documents

| Document                                             | Description                                           |
| ---------------------------------------------------- | ----------------------------------------------------- |
| [001-architecture.md](001-architecture.md)           | System architecture, component diagram, diff pipeline |
| [002-project-structure.md](002-project-structure.md) | Monorepo folder layout, workspace organization        |
| [003-rust-crates.md](003-rust-crates.md)             | Rust workspace, crate boundaries, module architecture |
| [004-protocols.md](004-protocols.md)                 | Wire protocols: shot, spot, health, story discovery   |
| [005-cli-design.md](005-cli-design.md)               | Multi-source CLI design, source types, error handling |
| [006-js-packages.md](006-js-packages.md)             | JavaScript client, Jest/Vitest integration            |
| [007-decisions.md](007-decisions.md)                 | Architecture decisions and resolved trade-offs        |

## User Documentation

| Document                                      | Description                                 |
| --------------------------------------------- | ------------------------------------------- |
| [CLI Reference](../docs/cli-reference.md)     | All commands and flags                      |
| [Configuration](../docs/configuration.md)     | Config file options and override precedence |
| [Getting Started](../docs/getting-started.md) | Installation and first use                  |
| [CI Integration](../docs/ci-integration.md)   | GitHub Actions, GitLab CI                   |

## Components

| Component | Crate          | Description                                                  |
| --------- | -------------- | ------------------------------------------------------------ |
| CLI       | `snapvrt`      | Orchestrator: discover, capture, compare, report             |
| Wire      | `snapvrt-wire` | Shared types and protocol constants                          |
| Shot      | `snapvrt-shot` | Screenshot + PDF worker (Chrome + PDFium, runs in container) |
| Spot      | `snapvrt-spot` | Diff service (pluggable engines, runs in container)          |

See [003-rust-crates.md](003-rust-crates.md) for crate details and module architecture.

## Directory Structure

```
.snapvrt/
├── config.toml
├── .gitignore
├── snapshots/
│   ├── storybook/
│   │   └── button-primary-desktop/
│   │       ├── reference.png        # Baseline (committed)
│   │       ├── current.png          # Current run (ignored)
│   │       └── diff.png             # Difference (ignored)
│   └── pdf/
│       └── invoice/
│           ├── manifest.json        # Metadata (committed)
│           ├── reference/           # Baseline pages (committed)
│           ├── current/             # Current run (ignored)
│           └── diff/                # Differences (ignored)
└── report.html                      # Visual comparison report (ignored)
```

Reference snapshots are committed; transient files are ignored via `.snapvrt/.gitignore`.

All file writes use atomic operations (temp file + rename).

## Installation

See [Getting Started](../docs/getting-started.md) for full installation guide.

```bash
# npm (recommended)
npm install -D snapvrt

# cargo
cargo install snapvrt
```

Requirements: Docker or compatible container runtime.

## Exit Codes

| Code | Meaning                                |
| ---- | -------------------------------------- |
| 0    | All tests passed (no new, no failures) |
| 1    | Visual differences or new snapshots    |
| 2    | Error (config, Docker, network, etc.)  |

## Container Configuration

### Container Launch

CLI uses bollard (Docker-compatible API) to spawn containers:

```bash
docker run \
  --rm -d \
  --shm-size=1g \
  --security-opt=seccomp=unconfined \
  --add-host=host.docker.internal:host-gateway \
  -e SNAPVRT_TABS=4 \
  -p ${PORT}:3000 \
  ghcr.io/snapvrt/shot
```

| Flag                                  | Purpose                                        |
| ------------------------------------- | ---------------------------------------------- |
| `--shm-size=1g`                       | Chrome needs shared memory for stability       |
| `--security-opt=seccomp=unconfined`   | Chrome sandboxing workaround (review post-MVP) |
| `--add-host=host.docker.internal:...` | Linux: map hostname to host gateway            |
| `-e SNAPVRT_TABS=4`                   | Number of concurrent browser tabs              |

### Container Images

| Image                              | Contents                           |
| ---------------------------------- | ---------------------------------- |
| `ghcr.io/snapvrt/shot`             | Rust binary + Chrome + PDFium      |
| `ghcr.io/snapvrt/spot-pixelmatch`  | Rust binary + pixelmatch (default) |
| `ghcr.io/snapvrt/spot-odiff`       | Rust binary + odiff                |
| `ghcr.io/snapvrt/spot-imagemagick` | Rust binary + imagemagick          |

See [002-project-structure.md](002-project-structure.md) for Dockerfile locations.

## Published Artifacts

| Registry  | Package                   | Purpose                                  |
| --------- | ------------------------- | ---------------------------------------- |
| npm       | `snapvrt`                 | Main package (downloads platform binary) |
| npm       | `@snapvrt/cli-{platform}` | Platform-specific binaries               |
| npm       | `@snapvrt/client`         | JS client for service API                |
| npm       | `@snapvrt/jest`           | Jest matchers                            |
| crates.io | `snapvrt`                 | CLI binary                               |
| GHCR      | `ghcr.io/snapvrt/shot`    | Screenshot + PDF service                 |
| GHCR      | `ghcr.io/snapvrt/spot-*`  | Diff services                            |

## Licensing

The main repository is MIT licensed. All bundled diff engines use MIT-compatible licenses (ISC, MIT, Apache-2.0).

Third-party engines with copyleft licenses (e.g., dssim/AGPL) are maintained in separate repositories. Users opt in by configuring the external image. See [003-rust-crates.md](003-rust-crates.md#licensing) for details.

## Out of Scope (v1)

- Storybook < 10
- React Native / mobile apps
- Non-Docker backends (traits defined, implementations future)
- Incremental testing / `--changed-since` (git-based story filtering)
