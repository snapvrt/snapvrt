# PoC: CDP Screenshot Capture (raw per-target WebSockets)

Direct CDP over per-target WebSockets using `tokio-tungstenite`. Each tab gets its
own dedicated connection (`ws://host:port/devtools/page/{targetId}`), enabling true
multi-tab parallelism in a single browser without library overhead.

See [CDP-COMPARISON.md](../CDP-COMPARISON.md) for benchmark results across all PoCs.

## Usage

```bash
# Single screenshot
cargo run -p poc-cdp-raw -- --url "http://localhost:6006/iframe.html?id=example-button--primary"

# 4 tabs in 1 browser, 8 stories round-robin
cargo run -p poc-cdp-raw -- --parallel 4

# 3 viewports (desktop/mobile/tablet) in parallel
cargo run -p poc-cdp-raw -- --test-viewports
```

## CLI options

```
--url <URL>          Single URL to screenshot
--output <PATH>      Output file path (default: screenshot.png)
--width <PX>         Viewport width (default: 1366)
--height <PX>        Viewport height (default: 768)
--scale <FACTOR>     Device scale factor (default: 1)
--parallel <N>       N tabs in 1 browser capturing 8 example stories
--test-viewports     3 viewports on 3 tabs in parallel
```

## Module structure

| Module       | Purpose                                                        |
| ------------ | -------------------------------------------------------------- |
| `chrome.rs`  | Launch Chrome, parse debug port, create tabs via CDP           |
| `cdp.rs`     | Per-target WebSocket client: `call()`, `wait_event()`, buffers |
| `capture.rs` | Screenshot pipeline (4 CDP domains, 6 commands)                |
| `main.rs`    | CLI with 3 modes                                               |

## Capture pipeline

1. `Emulation.setDeviceMetricsOverride` — viewport + scale
2. `Page.enable` + `Page.navigate` — navigate, wait for `Page.loadEventFired`
3. `Runtime.evaluate` — inject animation-disabling CSS
4. `Runtime.evaluate` — wait for fonts + DOM stability (100ms settle, `awaitPromise`)
5. `Runtime.evaluate` — `getBoundingClientRect()` on body
6. `Page.captureScreenshot` — PNG clipped to body
