# Getting Started

## Prerequisites

- Chrome (installed locally, or Docker for containerized mode)
- Storybook 10+
- Rust toolchain (for building from source)

## Install

```bash
# Build from source
cd rust && cargo build --release
# Binary at rust/target/release/snapvrt
```

npm distribution is planned but not yet available.

## Initialize

In your project root (where your Storybook lives):

```bash
snapvrt init --url http://localhost:6006
```

This creates `.snapvrt/config.toml` and `.snapvrt/.gitignore`.

## Capture Reference Snapshots

Start your Storybook, then:

```bash
# Start Storybook
npm run storybook

# In another terminal — capture baselines
snapvrt update
```

This discovers all stories, captures screenshots, and saves them as reference snapshots in `.snapvrt/reference/`. Commit these to git.

## Run Tests

```bash
snapvrt test
```

Compares current screenshots against references. Exit code 0 = all pass, 1 = any differences, new snapshots, or errors.

## Review Changes

```bash
# Generate HTML report and open in browser
snapvrt review --open
```

## Approve Changes

After reviewing, accept the changes:

```bash
# Approve all pending changes
snapvrt approve --all

# Or approve selectively
snapvrt approve --filter "button"
snapvrt approve --new        # only new snapshots
snapvrt approve --failed     # only failed snapshots
```

## Configuration

See [Configuration](configuration.md) for all config options. The defaults work for most projects:

```toml
# .snapvrt/config.toml
[source.storybook]
type = "storybook"
url = "http://localhost:6006"

[viewport.laptop]
width = 1366
height = 768
```

## Remote Chrome

To connect to a Chrome instance running elsewhere (e.g., in Docker):

```bash
# Start a headless Chrome container
docker run --rm -d -p 9222:9222 --shm-size=1g chromedp/headless-shell:latest

# Run snapvrt against it
snapvrt test --chrome-url http://localhost:9222
```

Or set it in config:

```toml
[capture]
chrome_url = "http://localhost:9222"
```
