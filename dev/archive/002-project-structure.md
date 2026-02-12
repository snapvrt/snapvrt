# Project Structure

Status: Accepted | Date: 2026-02-08

> Part of [snapvrt specification](README.md)

This document defines the monorepo folder structure for snapvrt.

## Design Decision: Language-First Organization

We use a **language-first** structure where each language ecosystem lives in its own self-contained folder.

**Rationale:**

- Clear boundaries between tech stacks
- Each folder is self-contained (`cd rust && cargo build`)
- Contributors specialize by language (Rust vs JS)
- Independent build/test/publish workflows per ecosystem
- Tooling isolation (no root-level Cargo.toml + package.json clutter)
- Future language clients (Python, Go) slot in naturally

## Folder Structure

```
snapvrt/
├── rust/                     # Rust workspace
│   ├── Cargo.toml            # Workspace manifest
│   ├── Cargo.lock
│   └── crates/
│       ├── snapvrt/          # CLI binary (published to crates.io)
│       ├── snapvrt-wire/     # Shared types & wire contract
│       ├── snapvrt-capture/  # Screenshot + PDF worker (container)
│       └── snapvrt-diff/     # Diff service (container)
│
├── node/                     # npm workspace
│   ├── package.json          # Workspace manifest
│   ├── pnpm-lock.yaml
│   ├── pnpm-workspace.yaml
│   └── packages/
│       ├── client/           # @snapvrt/client
│       ├── jest/             # @snapvrt/jest
│       └── vitest/           # @snapvrt/vitest
│
├── docker/                   # Container definitions
│   ├── capture/
│   │   └── Dockerfile
│   └── diff/
│       ├── dify/
│       │   └── Dockerfile
│       ├── odiff/
│       │   └── Dockerfile
│       └── imagemagick/
│           └── Dockerfile
│
├── examples/                 # Example projects
│   ├── storybook-basic/
│   ├── pdf-comparison/
│   └── jest-integration/
│
├── docs/                     # User-facing documentation
│   ├── getting-started.md
│   ├── configuration.md
│   ├── cli-reference.md
│   └── ci-integration.md
│
├── dev/                      # Internal design docs
│   ├── README.md             # Specification index
│   └── 0*.md                 # Numbered design docs
│
├── .github/                  # CI/CD workflows
│   └── workflows/
│
├── README.md
└── LICENSE
```

## Rust Workspace

```toml
# rust/Cargo.toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
repository = "https://github.com/snapvrt/snapvrt"
```

### Crates

| Crate             | Type    | Description                            |
| ----------------- | ------- | -------------------------------------- |
| `snapvrt`         | Binary  | CLI binary, HTTP server, orchestration |
| `snapvrt-wire`    | Library | Shared types & wire contract           |
| `snapvrt-capture` | Binary  | Screenshot + PDF worker (container)    |
| `snapvrt-diff`    | Binary  | Diff service (container)               |

See [003-rust-crates.md](003-rust-crates.md) for detailed crate architecture.

## Node Workspace

```yaml
# node/pnpm-workspace.yaml
packages:
  - "packages/*"
```

```json
// node/package.json
{
  "name": "snapvrt-node",
  "private": true,
  "packageManager": "pnpm@9.0.0"
}
```

### Packages

| Package           | Description                 |
| ----------------- | --------------------------- |
| `@snapvrt/client` | HTTP client for service API |
| `@snapvrt/jest`   | Jest integration            |
| `@snapvrt/vitest` | Vitest integration          |

See [006-js-packages.md](006-js-packages.md) for detailed package design.

## Containers

Container definitions live in `/docker/` rather than inside `/rust/` because:

- Containerfiles may pull from multiple sources (Rust binary + Node runtime)
- Easier to find for contributors unfamiliar with the codebase
- CI workflows reference a predictable location

```
docker/
├── capture/              # Screenshot + PDF service (Chrome + PDFium)
│   └── Dockerfile
└── diff/                 # Diff services (MIT-compatible engines only)
    ├── dify/             # MIT license (default)
    │   └── Dockerfile
    ├── odiff/            # MIT license
    │   └── Dockerfile
    └── imagemagick/      # Apache-2.0 license
        └── Dockerfile
```

Both services expose HTTP APIs (see [004-protocols.md](004-protocols.md)) and are managed by the orchestrator with idle timeouts.

Third-party engines (e.g., dssim/AGPL) are maintained in separate repositories.

## Examples

Example projects demonstrate real-world usage:

```
examples/
├── storybook-basic/          # Minimal Storybook setup
│   ├── .storybook/
│   ├── .snapvrt/
│   ├── package.json
│   └── src/
├── pdf-comparison/           # PDF testing with manifest
│   ├── .snapvrt/
│   ├── fixtures/
│   └── snapvrt-pdfs.json
└── jest-integration/         # Service mode with Jest
    ├── .snapvrt/
    ├── package.json
    └── tests/
```

## Future Language Clients

Additional clients would follow the same pattern:

```
snapvrt/
├── rust/
├── node/
├── python/                   # Future
│   ├── pyproject.toml
│   └── src/
│       └── snapvrt/
└── go/                       # Future
    ├── go.mod
    └── snapvrt/
```

## References

- [Graphite: Managing multiple languages in a monorepo](https://graphite.com/guides/managing-multiple-languages-in-a-monorepo)
- [push-based/multilanguage-monorepo](https://github.com/push-based/multilanguage-monorepo)
- [jihchi/dify](https://github.com/jihchi/dify) — Rust image diff tool distributed via npm with platform-specific binary packages. Evaluate as a reference for wrapping the `snapvrt` CLI binary and distributing it through npm (e.g., `npx snapvrt` or `npm install -g snapvrt`).
