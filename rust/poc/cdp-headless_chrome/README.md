# PoC: CDP Screenshot Capture (headless_chrome)

Synchronous CDP screenshot capture via [headless_chrome](https://docs.rs/headless_chrome).
Multi-tab works but is ~4x slower than sequential due to transport mutex contention.

See [CDP-COMPARISON.md](../CDP-COMPARISON.md) for benchmark results across all PoCs.

## Usage

```bash
# Single screenshot
cargo run -p poc-cdp-headless-chrome -- --url "http://localhost:6006/iframe.html?id=example-button--primary"

# 4 tabs in 1 browser, 8 stories round-robin
cargo run -p poc-cdp-headless-chrome -- --parallel 4
```

## CLI options

```
--url <URL>          Single URL to screenshot
--output <PATH>      Output file path (default: screenshot.png)
--width <PX>         Viewport width (default: 1366)
--height <PX>        Viewport height (default: 768)
--scale <FACTOR>     Device scale factor (default: 1)
--parallel <N>       N tabs capturing 8 example stories
```

## Capture pipeline

1. `Emulation.setDeviceMetricsOverride` — viewport + scale
2. `Page.navigate` — navigate to URL
3. `Runtime.evaluate` — inject animation-disabling CSS
4. `Runtime.evaluate` — wait for fonts + DOM stability (100ms settle)
5. `Runtime.evaluate` — `getBoundingClientRect()` on body
6. `Page.captureScreenshot` — PNG clipped to body
