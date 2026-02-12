# Docker-First Architecture

Status: Accepted | Date: 2026-02-10

> Part of [snapvrt specification](README.md)
>
> This document supersedes [001-architecture.md](001-architecture.md) and [003-rust-crates.md](003-rust-crates.md) for runtime architecture. Those documents remain as historical context.

## Summary

snapvrt is a single binary. By default it runs Chrome in a Docker container (`ghcr.io/snapvrt/chrome`) to capture screenshots via CDP, and diffs them in-process using dify. The container pins the browser version, fonts, and rendering environment — eliminating false positives from cross-platform differences.

For quick local iteration, `snapvrt test --local` skips Docker and uses whatever Chrome is installed on the host.

## Why Docker-First

### Visual regression demands environment consistency

snapvrt is a regression testing tool. Its job is to detect _unintended_ visual changes — not differences caused by the environment. Font rendering, antialiasing, Chrome version, and OS-level text shaping all affect pixel output. A button that renders identically in code can produce different pixels on macOS vs Linux vs CI.

If references are generated on one environment and compared on another, the result is noise — false positives that erode trust in the tool.

### Containers solve the consistency problem

A pinned Docker image (`ghcr.io/snapvrt/chrome`) guarantees:

- Exact Chrome version
- Exact font set (Liberation, Noto CJK, Noto Emoji)
- Exact rendering pipeline
- Identical output on macOS, Linux, Windows, CI

Developer laptops and CI produce byte-identical screenshots. References generated anywhere are valid everywhere.

### The cost is acceptable

- Container cold start adds 3–5s (amortized with long-running mode)
- Docker Desktop licensing applies to companies >250 employees (Colima/Podman are free alternatives)
- CI environments handle Docker natively (GitHub Actions, GitLab CI)
- The `--local` escape hatch exists for quick iteration

### What the market does

| Tool       | Default | Why                                     |
| ---------- | ------- | --------------------------------------- |
| Loki       | Docker  | Visual regression — consistency matters |
| Chromatic  | Cloud   | Controlled rendering environment        |
| BackstopJS | Local   | Docker optional                         |
| Playwright | Local   | E2E testing, not pixel regression       |
| Cypress    | Local   | E2E testing, not pixel regression       |
| Lost Pixel | Local   | Hybrid — supports Docker mode           |

Tools focused on _pixel-level regression_ (Loki, Chromatic) control the rendering environment. Tools focused on _functional testing_ (Playwright, Cypress) don't — because a 1px font difference doesn't fail a click-and-assert test.

snapvrt is a regression tool. Consistency is the feature.

## Architecture

### Default Mode: Docker

```
snapvrt test
│
├── 1. Discover stories (HTTP GET index.json)
├── 2. Start ghcr.io/snapvrt/chrome container (bollard)
│     └── Chrome + pinned fonts, CDP on port 9222
├── 3. Capture screenshots via CDP (N parallel tabs)
├── 4. Compare: memcmp → dify (in-process, spawn_blocking)
└── 5. Report results + write store + stop container

Chrome runs in a container. Everything else runs in-process.
```

### Local Mode: `--local`

```
snapvrt test --local
│
├── 1. Discover stories (HTTP GET index.json)
├── 2. Launch local Chrome (or connect to --chrome-url)
├── 3. Capture screenshots via CDP (N parallel tabs)
├── 4. Compare: memcmp → dify (in-process, spawn_blocking)
└── 5. Report results + write store

Everything runs in one process. No containers.
```

Use `--local` when:

- Iterating on component styles (don't need pixel-perfect references)
- Docker is unavailable or impractical
- You want the fastest possible feedback loop

**Warning:** `--local` results may differ from Docker results due to Chrome version, font, and OS differences. Don't mix local-generated references with Docker-generated comparisons.

### What Doesn't Need a Container

**Diffing.** dify runs in 44ms in-process. Serializing two PNGs over HTTP, sending them to a container, diffing, and serializing the result back would add more overhead than the computation itself. There is no cross-platform rendering concern — dify is pure math on pixel buffers, deterministic everywhere.

**Story discovery.** HTTP GET to a Storybook URL. No rendering involved.

**Store operations.** Filesystem reads/writes on the host.

## Docker Mode (Default)

### Quick Mode

Container starts before capture, stops after. This is what `snapvrt test` does out of the box.

```sh
# First run pulls the image automatically
snapvrt test
```

What happens under the hood:

1. Pull `ghcr.io/snapvrt/chrome:0.1.0` if not present (with progress bar)
2. `docker run --rm -d --shm-size=1g -p <random>:9222 ghcr.io/snapvrt/chrome:0.1.0`
3. Poll `http://localhost:<port>/json/version` until Chrome is ready (100ms intervals, 10s timeout)
4. Run the capture pipeline with `--chrome-url http://localhost:<port>`
5. `docker stop <container>` after completion (or on SIGINT)

Labels (`snapvrt.managed=true`, `snapvrt.session=<uuid>`) enable orphan cleanup.

### Long-Running Mode: `snapvrt docker`

For development iteration. Container stays up across runs, eliminating cold-start cost.

```sh
# Start Chrome container (stays running)
snapvrt docker start
# → Chrome running at http://localhost:9222

# Run tests against it (fast — no container startup)
snapvrt test
# Detects running container, skips start/stop

# When done
snapvrt docker stop
```

`snapvrt docker start`:

- Starts the container with an idle timeout (default: 30 min, configurable)
- Prints the Chrome URL
- Detaches (returns immediately)

`snapvrt docker stop`:

- Stops the managed container
- Cleans up

`snapvrt test` when a managed container is already running:

- Detects via Docker API (label query)
- Reuses it instead of starting a new one
- Does NOT stop it after the run

### Container Management (bollard)

Uses the bollard crate (Docker-compatible API) for container lifecycle:

```rust
// Simplified — actual implementation in pool.rs
async fn start_chrome_container(config: &DockerConfig) -> Result<ChromeContainer> {
    let docker = Docker::connect_with_defaults()?;

    // Check for already-running managed container
    if let Some(existing) = find_managed_container(&docker).await? {
        return Ok(existing);
    }

    // Pull image if needed
    ensure_image(&docker, &config.image).await?;

    // Start container
    let port = find_free_port()?;
    let container = docker.create_container(/* ... */).await?;
    docker.start_container(&container.id).await?;

    // Wait for Chrome to be ready
    wait_for_cdp(port).await?;

    Ok(ChromeContainer { id: container.id, port })
}
```

### Docker Config

```toml
# .snapvrt/config.toml

[docker]
# image = "ghcr.io/snapvrt/chrome:0.1.0"
# shm_size = "1g"
# idle_timeout = "30m"         # for `snapvrt docker start`
```

| Option                | Default                        | Description                             |
| --------------------- | ------------------------------ | --------------------------------------- |
| `docker.image`        | `ghcr.io/snapvrt/chrome:0.1.0` | Chrome container image                  |
| `docker.shm_size`     | `"1g"`                         | Shared memory size for Chrome           |
| `docker.idle_timeout` | `"30m"`                        | Auto-stop timeout for long-running mode |

### CLI

```
snapvrt test                   # Default: uses Docker
snapvrt test --local           # Skip Docker, use local Chrome
snapvrt update                 # Default: uses Docker
snapvrt update --local         # Skip Docker, use local Chrome

snapvrt docker start           # Start long-running Chrome container
snapvrt docker stop            # Stop it
snapvrt docker status          # Show running container info
```

The `--local` flag composes cleanly with all other flags:

```sh
snapvrt test --local --filter button -p 8
```

## Chrome Image: `ghcr.io/snapvrt/chrome`

A minimal image with exactly what's needed:

```dockerfile
FROM debian:bookworm-slim

# Chrome + dependencies
RUN apt-get update && apt-get install -y \
    chromium \
    fonts-liberation \
    fonts-noto-color-emoji \
    fonts-noto-cjk \
    --no-install-recommends \
    && rm -rf /var/lib/apt/lists/*

# Non-root user
RUN useradd -m chrome
USER chrome

EXPOSE 9222

ENTRYPOINT ["chromium", \
    "--headless=new", \
    "--disable-gpu", \
    "--no-sandbox", \
    "--no-first-run", \
    "--disable-extensions", \
    "--disable-background-networking", \
    "--disable-background-timer-throttling", \
    "--disable-backgrounding-occluded-windows", \
    "--disable-renderer-backgrounding", \
    "--disable-ipc-flooding-protection", \
    "--disable-sync", \
    "--disable-translate", \
    "--mute-audio", \
    "--hide-scrollbars", \
    "--remote-debugging-address=0.0.0.0", \
    "--remote-debugging-port=9222"]
```

**Why our own image:**

- Pin exact Chrome version (reproducible across CI runs)
- Pin exact font set (consistent text rendering)
- Minimal attack surface (no extra tools, no package manager cache)
- Correct Chrome flags for screenshot capture (no guessing image defaults)
- Non-root by default

**Versioning:** Image version tracks snapvrt version. `snapvrt 0.1.0` uses `ghcr.io/snapvrt/chrome:0.1.0`. The image tag is compiled into the binary as the default.

## Crate Structure

Single crate. No `snapvrt-wire`, no `snapvrt-capture`, no `snapvrt-diff`.

```
rust/crates/snapvrt/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── cli.rs              # Clap definitions
    ├── config.rs           # TOML loading + merge
    ├── discover.rs         # Storybook index.json
    ├── store.rs            # .snapvrt/ filesystem ops
    ├── report.rs           # Terminal output + timings
    ├── review.rs           # HTML report generation
    ├── runner/
    │   ├── mod.rs          # init, review, open_in_browser
    │   ├── capture_run.rs  # CaptureRun: plan + execute
    │   ├── test.rs         # snapvrt test
    │   ├── update.rs       # snapvrt update
    │   └── approve.rs      # snapvrt approve
    ├── capture/
    │   ├── mod.rs          # capture_all orchestration
    │   ├── chrome.rs       # Chrome launch / connect
    │   ├── cdp.rs          # CDP WebSocket transport
    │   └── pipeline/       # Pipeline + strategies
    │       ├── mod.rs
    │       ├── config.rs
    │       ├── pipeline.rs
    │       ├── types.rs
    │       ├── animation.rs
    │       ├── clip.rs
    │       ├── js.rs
    │       ├── network.rs
    │       └── screenshot.rs
    ├── diff/
    │   ├── mod.rs
    │   └── diff.rs         # In-process dify
    └── docker/
        ├── mod.rs          # DockerManager
        └── container.rs    # bollard operations
```

**Why no separate crates:**

- `snapvrt-wire` was for shared types between services. With everything in-process, types are just `pub(crate)` structs.
- `snapvrt-capture` was a container binary. The capture pipeline runs in the CLI process.
- `snapvrt-diff` was a container binary. dify runs in-process in 44ms.

Separate crates add compile-time cost, dependency management overhead, and protocol versioning complexity — all for no benefit when everything shares a process.

## What Stays the Same

These parts of the original spec are unchanged:

- **CLI commands** — `init`, `test`, `update`, `approve`, `review`, `prune` (005, CLI reference)
- **Exit codes** — 0 (pass), 1 (fail/new), 2 (error)
- **Config format** — `.snapvrt/config.toml` (with additions for `[docker]`)
- **Store layout** — `.snapvrt/{reference,current,difference}/`
- **Story discovery** — `index.json`, filter by type, `snapvrt-skip` tag
- **Capture pipeline** — 9 stages, strategy pattern, presets (standard/loki)
- **2-phase diff** — memcmp → dify with YIQ threshold + anti-aliasing
- **Diff engine** — dify as default, same threshold semantics
- **Ready detection** — fonts + DOM mutation + network idle
- **Screenshot stability** — up to 3 shots, byte-compare consecutive
- **Loki compatibility** — preset for pixel-identical captures
- **HTML report** — static file-referenced images
- **Wire protocols** (004) — still the target if capture/diff ever become services (v1.1+)

## What Changes From the Original Spec

| Aspect                | Original (001/003)                                               | Docker-First (010)                               |
| --------------------- | ---------------------------------------------------------------- | ------------------------------------------------ |
| Default runtime       | Containers (Docker required)                                     | Docker (single Chrome container)                 |
| Capture service       | `snapvrt-capture` binary in container, HTTP API                  | In-process CDP, Chrome in container via bollard  |
| Diff service          | `snapvrt-diff` binary in container, HTTP API                     | In-process dify, same binary                     |
| Crate count           | 4 (`snapvrt`, `snapvrt-wire`, `snapvrt-capture`, `snapvrt-diff`) | 1 (`snapvrt`)                                    |
| Container images      | `ghcr.io/snapvrt/capture`, `ghcr.io/snapvrt/diff-*`              | `ghcr.io/snapvrt/chrome` (Chrome only)           |
| What's containerized  | Everything (capture + diff)                                      | Only Chrome (capture env). Diff runs in-process  |
| Multiple diff engines | Different container images per engine                            | In-process dify only (others via config, future) |
| Service mode (v1.1)   | HTTP API wrapping orchestrator                                   | Deferred, design unchanged                       |
| `WorkerPool` trait    | Core abstraction, DockerPool + StaticPool                        | Not needed for v1 (direct CDP)                   |
| Local mode            | Not available                                                    | `--local` flag bypasses Docker                   |

## Roadmap Impact

The original roadmap phases shift:

| Phase                    | Original                                                                       | Docker-First                                                                  |
| ------------------------ | ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------- |
| Phase 1 (Foundation)     | Build `snapvrt-wire`, `snapvrt-capture`, `snapvrt-diff` crates + Docker images | **Already done** — capture pipeline + dify work in-process                    |
| Phase 2 (CLI Core)       | Build orchestrator, pool, config, store, compare                               | **Mostly done** — runner, store, config, compare exist                        |
| Phase 3 (CLI Commands)   | init, test, update, approve, prune                                             | **Mostly done** — init, test, update, approve work. Prune pending             |
| Phase 4 (Output)         | Reporter, HTML report, review UI                                               | **Partially done** — terminal reporter + HTML report exist. Review UI pending |
| Phase 5 (PDF)            | PDF endpoint in snapvrt-capture                                                | In-process PDF rendering (pdfium-render). Not started                         |
| Phase 6 (Service v1.1)   | HTTP API, JS packages                                                          | Unchanged, deferred                                                           |
| **Next: Docker default** | N/A                                                                            | Wire bollard into default pipeline, build Chrome image, add `--local` flag    |

## Open Questions

1. **Windows Chrome discovery.** For `--local` mode, `find_chrome()` currently only handles macOS and Linux paths. Windows needs registry lookup or `where chrome` equivalent.

2. **Chrome version pinning in local mode.** Should snapvrt warn when the local Chrome version differs from the container version? This could help users understand why local results might differ from Docker results.

3. **Multiple diff engines.** The original spec planned odiff and imagemagick as container-based alternatives. In the single-binary architecture, these would be compile-time features or separate binaries. Is this needed for v1?

4. **Podman/Colima compatibility.** Docker Desktop licensing concerns mean some teams use alternatives. bollard supports the Docker API, but we should test and document Podman and Colima explicitly.
