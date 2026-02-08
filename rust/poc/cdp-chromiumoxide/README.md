# PoC: CDP Screenshot Capture (chromiumoxide)

Async CDP screenshot capture via [chromiumoxide](https://github.com/nickel-org/chromiumoxide)
(tokio). Uses a browser-pool pattern for parallelism — multi-tab in one browser is
broken in chromiumoxide 0.8 due to single-handler bottleneck.

See [CDP-COMPARISON.md](../CDP-COMPARISON.md) for benchmark results across all PoCs.

## Usage

```bash
# Single screenshot
cargo run -p poc-cdp-chromiumoxide -- --url "http://localhost:6006/iframe.html?id=example-button--primary"

# 4 browser instances, 8 stories round-robin
cargo run -p poc-cdp-chromiumoxide -- --parallel 4

# 3 viewports (desktop/mobile/tablet) in parallel
cargo run -p poc-cdp-chromiumoxide -- --test-viewports
```

## CLI options

```
--url <URL>          Single URL to screenshot
--output <PATH>      Output file path (default: screenshot.png)
--width <PX>         Viewport width (default: 1366)
--height <PX>        Viewport height (default: 768)
--scale <FACTOR>     Device scale factor (default: 1)
--parallel <N>       N browser instances capturing 8 example stories
--test-viewports     3 viewports on 3 browsers in parallel
```

## Capture pipeline

1. `Emulation.setDeviceMetricsOverride` — viewport + scale (per-capture)
2. `Page.navigate` — navigate to URL
3. `CSS.createStyleSheet` + `CSS.setStyleSheetText` — disable animations
4. `Runtime.evaluate` — wait for fonts + DOM stability (100ms settle)
5. `DOM.getDocument` + `DOM.querySelector` + `DOM.getBoxModel` — body bounds
6. `Page.captureScreenshot` — PNG clipped to body
