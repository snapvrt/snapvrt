# Roadmap

## Completed

### Phase 0: Validate (PoCs) ✅

Four PoCs validated core assumptions. All findings integrated into the main crate.

| PoC | What                         | Outcome                                                         |
| --- | ---------------------------- | --------------------------------------------------------------- |
| 0.1 | Storybook 10 example project | `examples/storybook-basic/` — validates index.json API          |
| 0.2 | CDP screenshot capture       | cdp-raw won — per-target WebSockets, true multi-tab parallelism |
| 0.3 | Storybook source discovery   | index.json parsing, story filtering, `snapvrt-skip` tag         |
| 0.4 | Image diff engine comparison | dify won — YIQ perceptual + anti-aliasing, MIT, 44ms            |

PoC code lives in git history (commits #2-#5). Production code is in `rust/crates/snapvrt/`.

### Phase 1-3: Core CLI ✅

Architecture pivoted from 4-crate microservices to single binary with in-process capture and diff. See `dev/archive/010-docker-first.md` for the rationale.

**What's working:**

- Config system (TOML file + env vars + CLI flags, with validation)
- Storybook discovery (index.json fetch, story filtering, iframe URL building)
- CDP integration (local Chrome launch, remote connect via `--chrome-url`, tab management)
- 9-stage capture pipeline (viewport, navigate, load, network idle, animations, ready, story root, clip, screenshot)
- Screenshot stability (up to 3 consecutive byte-identical shots)
- Web Animations API control (finish finite, cancel infinite)
- 2-phase diff (memcmp fast path → dify perceptual diff)
- Dimension mismatch handling (magenta padding)
- Snapshot store (`.snapvrt/{reference,current,difference}/`)
- All 6 CLI commands: `init`, `test`, `update`, `approve`, `review`, `prune`
- Terminal reporter (color output, progress, timing tables, actionable summary)
- Static HTML report generation

**~3,500 lines of Rust** across 32 source files.

### Phase 4: Output (Partial) ✅

- Terminal reporter: done
- Static HTML report: done
- Review as live HTTP server: not done (review generates static HTML, not a server with approve actions)

---

## Next Up

### Docker Integration

**Priority: highest.** This is the gap between "works on developer's machine" and "works for users."

Currently the tool either launches a local Chrome or connects to `--chrome-url`. Docker integration makes it zero-config.

- [ ] Add `bollard` dependency for Docker API
- [ ] Build `ghcr.io/snapvrt/chrome` image (Debian slim + Chromium + pinned fonts)
- [ ] `docker/` module: pull image, start/stop container, health check polling
- [ ] Quick mode (default): auto-start Chrome container before capture, auto-stop after
- [ ] Long-running mode: `snapvrt docker start/stop/status` for faster iteration
- [ ] `--local` flag: current behavior (launch local Chrome, no container)
- [ ] Container labels for orphan cleanup (`snapvrt.managed=true`, `snapvrt.session=<uuid>`)

**Milestone:** `snapvrt test` works out of the box with only Docker installed. No manual Chrome setup.

### Test Infrastructure

Currently only `compare/diff.rs` has tests. Priority areas:

- [ ] Config parsing (unit tests for validation, merge, defaults)
- [ ] Store operations (unit tests for read/write/list/clean)
- [ ] Storybook discovery (integration test with mock HTTP server)
- [ ] Full `snapvrt test` against `examples/storybook-basic/` (e2e)
- [ ] CI pipeline (GitHub Actions)

### npm Distribution

Distribute the binary via npm for lower adoption friction (`npx snapvrt`). Reference: dify project uses platform-specific binary packages.

- [ ] `snapvrt` npm package (optionalDependencies on platform packages)
- [ ] `@snapvrt/cli-{platform}` packages (darwin-arm64, darwin-x64, linux-x64, linux-arm64)
- [ ] Post-install script that verifies binary works

---

## Later

### PDF Support

In-process PDF rendering via pdfium-render (BSD-3, same engine Chrome uses).

- [ ] Add `pdfium-render` dependency
- [ ] PDF → PNG rendering (per-page)
- [ ] PDF manifest file (`snapvrt-pdfs.json`) support
- [ ] Multi-page snapshot storage
- [ ] Page count change detection

### Review UI Improvements

- [ ] `snapvrt review` as live HTTP server (not just static HTML)
- [ ] Approve actions from the review UI
- [ ] WebSocket for live updates during test runs

### Service Mode (v1.1)

HTTP API wrapping the CLI for programmatic use from test frameworks.

- [ ] `snapvrt service start/stop` — HTTP server (axum)
- [ ] `@snapvrt/client` — JS HTTP client
- [ ] `@snapvrt/jest` — Jest async matchers (`toMatchPdfSnapshot`, `toMatchWebSnapshot`)
- [ ] `@snapvrt/vitest` — Vitest async matchers

### Nice to Have

- Storybook error detection (`.sb-show-errordisplay` check after navigation)
- Element masking (cover dynamic elements with solid overlays before capture)
- Per-story configuration via `parameters.snapvrt` in story metadata
- `--auto-add-new` flag for development convenience
- Multi-source support (multiple Storybook instances, web pages)
- `--fail-fast` flag
- Podman/Colima documentation and testing

---

## Research References

| Doc                             | Purpose                                                   |
| ------------------------------- | --------------------------------------------------------- |
| `dev/008-screenshot-capture.md` | Screenshot capture best practices survey across OSS tools |
| `dev/009-audit.md`              | Full audit of docs vs implementation (2026-02-12)         |
| `dev/archive/`                  | Original design docs (superseded by architecture pivot)   |
