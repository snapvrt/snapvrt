# snapvrt

Snap. Test. Ship.

Visual regression testing for Storybook 10+ and PDFs.

> **Status:** In development. Not yet ready for use.

## Why snapvrt

Inspired by [loki](https://loki.js.org), with a different approach:

- **Multi-source** - Storybook, PDFs, and more (not just Storybook)
- **Storybook 10+ only** - No legacy API support, cleaner integration
- **Rust CLI + Docker** - Single binary, consistent screenshots across platforms
- **Interactive review** - Browser-based diff viewer out of the box
- **Service mode** - HTTP API for Jest/Vitest integration

If you need Storybook 5-8, use loki.

## Quick Start

> Not yet published.

```bash
npm install -D snapvrt     # or: cargo install snapvrt

snapvrt init               # create .snapvrt/ config
snapvrt update             # capture reference screenshots
snapvrt test               # run visual regression tests
snapvrt review             # review changes in browser
```

## Documentation

- [Getting Started](docs/getting-started.md) - Installation and first use
- [Configuration](docs/configuration.md) - Config file reference
- [CLI Reference](docs/cli-reference.md) - All commands and flags
- [CI Integration](docs/ci-integration.md) - GitHub Actions, GitLab CI
- [Design Docs](dev/) - Architecture and specification

## Contributing

Prerequisites: Rust (see `rust/rust-toolchain.toml`), Node.js (see `.nvmrc`), Docker, pnpm.

```bash
cd rust && cargo build                    # build CLI + workers
cd node && pnpm install && pnpm build     # build JS packages
```

See [dev/](dev/) for design docs and architecture before implementing.

## License

MIT
