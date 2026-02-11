# snapvrt (Rust)

## Prerequisites

- Rust toolchain (stable)
- A running Storybook instance (default: `http://localhost:6006`) — supports Storybook 8, 9, and 10
- Chrome/Chromium installed (or Docker for cross-platform consistency)

## Build

```sh
cargo build -p snapvrt
```

## Usage

All commands run from the `rust/` directory:

```sh
# Initialize config (.snapvrt/config.toml)
cargo run -p snapvrt -- init
cargo run -p snapvrt -- init --url http://localhost:6006

# Update reference snapshots
cargo run -p snapvrt -- update

# Run visual regression tests (exit 0 = pass, 1 = fail)
cargo run -p snapvrt -- test

# Filter by name (case-insensitive substring match)
cargo run -p snapvrt -- test -f button
cargo run -p snapvrt -- update -f button

# Control parallelism
cargo run -p snapvrt -- test --parallel 4

# Show per-snapshot timing breakdown
cargo run -p snapvrt -- test --timings

# Generate HTML review report
cargo run -p snapvrt -- review
cargo run -p snapvrt -- review --open
```

## Docker Chrome (cross-platform screenshots)

Run Chrome in Docker for consistent rendering across hosts. Works on Linux and macOS with the same command:

```sh
docker run -d --name snapvrt-chrome -p 9222:9222 --shm-size=4g \
  --cap-add=SYS_ADMIN \
  yukinying/chrome-headless-browser-stable:139.0.7258.138 \
  --disable-background-networking \
  --disable-gpu \
  --disable-software-rasterizer \
  --disable-extensions \
  --no-first-run \
  --hide-scrollbars
```

Then pass `--chrome-url`:

```sh
cargo run -p snapvrt -- update --chrome-url http://localhost:9222
cargo run -p snapvrt -- test --chrome-url http://localhost:9222
```

Or set it permanently in `.snapvrt/config.toml`:

```toml
[shot]
chrome_url = "http://localhost:9222"
```

Stop the container:

```sh
docker stop snapvrt-chrome
docker rm snapvrt-chrome
```

| Docker flag | Purpose |
| --- | --- |
| `-p 9222:9222` | Expose CDP port to host |
| `--shm-size=4g` | Chrome needs shared memory for stability |
| `--cap-add=SYS_ADMIN` | Chrome sandboxing workaround |
| `--disable-gpu` | Prevent GPU rendering variance |
| `--disable-software-rasterizer` | Prevent GPU process crash loop under emulation (e.g. Docker on Apple Silicon) |
| `--hide-scrollbars` | Prevent scrollbars in captures |

When `--chrome-url` is set, localhost URLs in story paths are automatically rewritten to the host's real LAN IP address so Chrome inside Docker can reach the host's Storybook. If IP detection fails (e.g. no network), it falls back to `host.docker.internal`.

## Shot pipeline options

Override via CLI flags or in `.snapvrt/config.toml` under `[shot]`:

| Flag                | Values                  | Default      | Description                           |
| ------------------- | ----------------------- | ------------ | ------------------------------------- |
| `--preset`          | `standard`, `loki`      | `standard`   | Base strategy preset                  |
| `--animation`       | `post-load`, `loki`     | `post-load`  | Animation disabling strategy          |
| `--clip`            | `story-root`, `body`    | `story-root` | Clip region for screenshots           |
| `--screenshot`      | `stable`, `single`      | `stable`     | Stability-check loop or single shot   |
| `--network-wait`    | `none`, `idle`, `fixed` | `idle`       | Network idle detection before capture |
| `--parallel` / `-p` | number                  | `4`          | Concurrent browser tabs               |
| `--chrome-url`      | URL                     | (local)      | Remote Chrome CDP endpoint            |
| `--timings`         | flag                    | off          | Print per-snapshot timing table       |

## Debug logging

Uses `tracing` via `RUST_LOG`:

```sh
# Full debug output
RUST_LOG=snapvrt=debug cargo run -p snapvrt -- update

# Trace (includes network idle internals)
RUST_LOG=snapvrt=trace cargo run -p snapvrt -- update

# Warnings only
RUST_LOG=snapvrt=warn cargo run -p snapvrt -- test
```
