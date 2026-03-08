# snapvrt

Visual regression testing for Storybook. Snap. Test. Ship.

## Design Principles

1. **Rust + MIT** — single binary CLI, permissive license
2. **Docker-first** — Chrome runs in a container for consistent rendering across platforms
3. **Storybook 10 only** — no legacy API support
4. **Zero configuration** — sensible defaults, minimal setup
5. **Fast** — parallel capture, in-process diffing

## Documentation

### Architecture & Roadmap

| Document                           | Description                                                                       |
| ---------------------------------- | --------------------------------------------------------------------------------- |
| [architecture.md](architecture.md) | System architecture, crate structure, capture pipeline, diff engine, store layout |
| [ROADMAP.md](ROADMAP.md)           | What's done, what's next (Docker integration), what's later                       |

### Research

| Document                                               | Description                                               |
| ------------------------------------------------------ | --------------------------------------------------------- |
| [008-screenshot-capture.md](008-screenshot-capture.md) | Screenshot capture best practices survey across OSS tools |
| [009-audit.md](009-audit.md)                           | Full docs-vs-implementation audit (2026-02-12)            |

### User Documentation

| Document                                      | Description                                |
| --------------------------------------------- | ------------------------------------------ |
| [CLI Reference](../docs/cli-reference.md)     | All commands and flags                     |
| [Configuration](../docs/configuration.md)     | Config file format and override precedence |
| [Getting Started](../docs/getting-started.md) | Installation and first use                 |
| [CI Integration](../docs/ci-integration.md)   | GitHub Actions setup                       |

## Quick Reference

**Current state:** Core CLI works end-to-end for Storybook. Docker integration is next.

**Exit codes:** 0 (pass), 1 (fail/new/error)

**Store layout:**

```
.snapvrt/
├── config.toml
├── reference/    ← committed
├── current/      ← gitignored
└── difference/   ← gitignored
```

**License:** MIT
