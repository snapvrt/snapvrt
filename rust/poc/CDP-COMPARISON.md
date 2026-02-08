# CDP PoC Comparison: chromiumoxide vs headless_chrome vs cdp-raw

Benchmark results from the three CDP screenshot PoCs, testing concurrency strategies
for the production `snapvrt-shot` capture service.

## Test setup

- macOS, M-series
- Storybook basic example (8 simple stories)
- Chrome auto-detected by each crate
- Per-story capture: navigate + CSS inject + ready wait + body clip + screenshot

## Results (8 stories, macOS M-series)

| Library         | Strategy      | Tabs | Browsers | Wall time | Peak RSS | Status |
| --------------- | ------------- | ---: | -------: | --------: | -------: | ------ |
| chromiumoxide   | Sequential    |    1 |        1 |       ~1s |  ~1.2 GB | ok     |
| chromiumoxide   | Multi-tab     |    4 |        1 |      30s+ |        — | broken |
| chromiumoxide   | Browser-pool  |    4 |        4 |       ~1s |  ~4.3 GB | ok     |
| headless_chrome | Sequential    |    1 |        1 |       ~2s |        — | ok     |
| headless_chrome | Multi-tab     |    4 |        1 |       ~8s |  ~1.8 GB | slow   |
| **cdp-raw**     | Per-target WS |    1 |        1 |     ~2.2s | ~1.35 GB | ok     |
| **cdp-raw**     | Per-target WS |    4 |        1 |     ~1.3s |  ~1.8 GB | ok     |
| **cdp-raw**     | Per-target WS |    8 |        1 |     ~1.2s |  ~2.6 GB | ok     |

## Analysis

### chromiumoxide multi-tab is broken

chromiumoxide 0.8 serializes all CDP traffic through a single `Handler::poll_next()`
loop ([source](https://github.com/mattsse/chromiumoxide/blob/main/src/handler/mod.rs)).
Three architectural bottlenecks compound:

1. **`channel(1)` buffers** -- Browser-to-handler and page-to-target channels have
   capacity 1, creating a convoy effect when multiple tabs send commands
2. **Sequential target polling** -- The handler iterates all targets in one loop
   iteration, adding O(N) latency per CDP round-trip
3. **No WebSocket pipelining** -- Commands are sent one at a time with a flush
   between each

Confirmed by upstream issues
[#235](https://github.com/mattsse/chromiumoxide/issues/235) (navigation timeouts
with multiple tabs) and [#237](https://github.com/mattsse/chromiumoxide/issues/237)
(request for command batching).

### headless_chrome multi-tab works but is slow

headless_chrome uses `Arc<Tab>` (Send + Sync) with a shared `Arc<Transport>`.
Multiple OS threads can call tab methods concurrently. The transport has internal
locking (mutex on the WebSocket) which serializes CDP at the wire level, but doesn't
deadlock. The ~4x slowdown vs sequential suggests per-command lock contention rather
than the total architectural breakdown seen in chromiumoxide.

### Browser-pool bypasses all transport issues

Separate Chrome processes each get their own WebSocket connection, handler/transport,
and process isolation. No contention at any layer. The cost is ~1.1 GB per browser
instance (~9 OS processes each: main, GPU, utility, renderer, etc.).

### Per-target WebSockets: best of both worlds

cdp-raw connects directly to each tab's dedicated WebSocket
(`ws://host:port/devtools/page/{targetId}`). Each tab gets its own `CdpConnection`
with no shared transport, no multiplexing, no contention — true multi-tab parallelism
in a single browser process.

Key results:

- **1.7x speedup** with 4 tabs vs sequential (~1.3s vs ~2.2s)
- **~150-180 MB per tab** vs ~1.1 GB per browser in the browser-pool approach
- **Diminishing returns at 8 tabs** for 8 trivial stories (~1.2s vs ~1.3s) — Chrome
  launch + tab setup dominates when per-capture work is small
- Only ~300 lines of custom CDP transport (tokio-tungstenite + serde_json)

Critical Chrome flags required for multi-tab parallelism:

- `--disable-background-timer-throttling` — prevents Chrome from throttling
  `setTimeout` to 1s minimum in background tabs (breaks ready-wait JS)
- `--disable-renderer-backgrounding` — prevents renderer priority reduction
- `--disable-backgrounding-occluded-windows` — prevents occluded window throttling
- `--disable-ipc-flooding-protection` — prevents CDP message rate-limiting

Without these flags, parallel captures are _slower_ than sequential because Chrome
throttles the 100ms DOM-settle timer in `WAIT_FOR_READY_JS` to ~1s per background tab.
chromiumoxide adds these flags by default; cdp-raw must set them explicitly.

## Recommendations for production

| Strategy          | Parallelism | Peak RSS | Wall time | Speedup | Docker memory | Docker CPU |
| ----------------- | ----------: | -------: | --------: | ------: | ------------: | ---------: |
| **Per-target WS** |       1 tab | ~1.35 GB |     ~2.2s |      1x |          2 GB |     1 core |
| **Per-target WS** |      4 tabs |  ~1.8 GB |     ~1.3s |    1.7x |        2.5 GB |    2 cores |
| **Per-target WS** |      8 tabs |  ~2.6 GB |     ~1.2s |    1.8x |        3.5 GB |    4 cores |
| Browser-pool      |  2 browsers |  ~2.4 GB |       ~1s |      2x |          3 GB |    2 cores |
| Browser-pool      |  4 browsers |  ~4.3 GB |       ~1s |      4x |          5 GB |    4 cores |

Per-target WS scales at ~150-180 MB/tab vs ~1.1 GB/browser. For heavier pages or
larger story counts, the speedup will be more pronounced as per-capture work
dominates over launch overhead.

Requires `--shm-size=2g` or `-v /dev/shm:/dev/shm` for Chrome's shared memory.
