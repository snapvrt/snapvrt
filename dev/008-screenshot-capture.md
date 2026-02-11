# 008 — Screenshot Capture: Best Practices Analysis

> Comparison of screenshot capture techniques across OSS visual regression
> testing tools, with recommendations for snapvrt.

## Tools Surveyed

| Tool                 | Capture Method                          | Protocol                     | Open Source |
| -------------------- | --------------------------------------- | ---------------------------- | ----------- |
| **Playwright**       | Direct pixel (CDP/Firefox/WebKit)       | CDP `Page.captureScreenshot` | Yes         |
| **Loki**             | Direct pixel (CDP)                      | CDP `Page.captureScreenshot` | Yes         |
| **Cypress**          | Direct pixel (CDP)                      | CDP `Page.captureScreenshot` | Yes         |
| **Chromatic**        | Cloud browsers                          | CDP (proprietary cloud)      | No          |
| **BackstopJS**       | Direct pixel (Puppeteer/Playwright)     | CDP `Page.captureScreenshot` | Yes         |
| **Percy**            | DOM serialization + cloud render        | N/A (HTML upload)            | CLI only    |
| **storycap-testrun** | Direct pixel (Playwright + CDP metrics) | CDP `Performance.getMetrics` | Yes         |
| **Happo**            | Cloud browser workers                   | Undocumented                 | CLI only    |

Additional tools reviewed: Applitools Eyes (DOM snapshot + Ultrafast Grid),
Lost Pixel (Playwright `page.screenshot()`), reg-suit (capture-agnostic diffing).

---

## Technique Comparison

### 1. Screenshot Stability (Burst Capture)

Take N screenshots and wait for two consecutive identical results before
accepting. Catches timing issues that heuristics miss.

| Tool                 | Implementation                                                                                                |
| -------------------- | ------------------------------------------------------------------------------------------------------------- |
| **Playwright**       | `toHaveScreenshot()` takes screenshots until two match (configurable `maxDiffPixelRatio`, retries with delay) |
| **Chromatic**        | SteadySnap: continuously captures until two consecutive match; flags snapshot if timeout exceeded             |
| **storycap-testrun** | Hash-based: captures multiple screenshots, compares hashes, retries if mismatch                               |
| **Others**           | No stability check — rely on readiness heuristics + fixed delays                                              |

**Impact:** High. This is the single most effective technique for eliminating
flaky screenshots. It acts as a safety net for all other readiness checks.

### 2. Page Readiness Detection

| Technique                      | Who Uses It                     | How                                                 |
| ------------------------------ | ------------------------------- | --------------------------------------------------- |
| `Page.loadEventFired`          | All CDP-based tools             | Wait for HTML `load` event                          |
| Network idle                   | Loki, snapvrt, Chromatic, Percy | Track in-flight requests; settle after quiet period |
| `document.fonts.ready`         | Playwright, snapvrt             | Wait for font face loading promise                  |
| DOM mutation observer          | snapvrt                         | Observe mutations; settle after quiet period        |
| CDP `Performance.getMetrics`   | storycap-testrun                | Monitor layout counts, node counts, paint events    |
| `readySelector` / `readyEvent` | BackstopJS, Loki                | User-defined DOM selector or console.log signal     |
| Fixed delay                    | Lost Pixel (1s), BackstopJS     | Simple `sleep()` before capture                     |

**snapvrt's current approach** (network idle + fonts + DOM mutation) is
already best-in-class for automatic detection. The main gap is the lack
of a stability check as a final safety net.

### 3. Animation & Transition Control

#### CSS Injection

| Tool          | When                                         | Method                                                                                 |
| ------------- | -------------------------------------------- | -------------------------------------------------------------------------------------- |
| **Loki**      | Pre-navigation                               | `Page.addScriptToEvaluateOnNewDocument` — injects via `insertRule` on DOMContentLoaded |
| **snapvrt**   | Pre-nav (loki-compat) or post-load (default) | Same as Loki in compat mode; `<style>` injection in default mode                       |
| **Cypress**   | Pre-screenshot                               | Injects `<style>` with `animation-duration: 0s; transition-property: none`             |
| **Percy**     | Cloud render                                 | Injects `animation: none; transition: none` during rendering                           |
| **Chromatic** | Cloud render                                 | Pauses CSS animations at last frame (configurable to first)                            |

#### JavaScript Animation Control

| Tool           | Technique                                                                                     |
| -------------- | --------------------------------------------------------------------------------------------- |
| **Playwright** | `document.getAnimations().forEach(a => a.finish() or a.cancel())` — Web Animations API        |
| **Loki**       | `requestAnimationFrame` monkey-patch: fast-forwards 1000 frames; `performance.now()` override |
| **Cypress**    | Stores and suspends `setTimeout`, `setInterval`, `requestAnimationFrame` callbacks            |
| **Chromatic**  | No automatic JS animation control — users call `isChromatic()` to conditionally disable       |

**Gap:** snapvrt disables CSS animations but does not handle in-progress
JS-driven animations (framer-motion, GSAP, Web Animations API). Playwright's
approach of calling `animation.finish()` / `animation.cancel()` via
`document.getAnimations()` is the most precise solution.

### 4. Viewport & Clipping

#### Full-Page Capture for Tall Content

| Tool               | Approach                                                                             |
| ------------------ | ------------------------------------------------------------------------------------ |
| **Playwright**     | `captureBeyondViewport: true` CDP param — captures natively without resize           |
| **Loki / snapvrt** | Resize viewport via `Emulation.setDeviceMetricsOverride` + 500ms delay, then restore |
| **Cypress**        | Scroll-and-stitch (multiple viewport captures stitched together)                     |
| **BackstopJS**     | Puppeteer `fullPage: true` or scroll-and-stitch                                      |

**Gap:** The viewport resize approach has three problems:

1. **500ms delay** per tall story (adds up with many stories)
2. **Layout shifts** — resizing the viewport can trigger responsive breakpoints
3. **Restore overhead** — need to reset viewport after capture

CDP's `captureBeyondViewport: true` eliminates all three issues. It captures
the page as-is, beyond the viewport boundary, without any resize.

#### Clip Region Calculation

| Tool               | Method                                                              |
| ------------------ | ------------------------------------------------------------------- |
| **Loki / snapvrt** | Walk visible children of Storybook root, union their bounding rects |
| **BackstopJS**     | `querySelector` on configured selector, use `getBoundingClientRect` |
| **Cypress**        | Element-level: clip to element rect; full-page: entire document     |
| **Playwright**     | Element-level: clip to element handle's bounding box                |

snapvrt's clip calculation (matching Loki's `getSelectorBoxSize`) is already
the most sophisticated approach.

### 5. Element Masking

Cover dynamic elements (timestamps, avatars, ads) with solid-color
overlays before screenshot to reduce false positives.

| Tool                 | Implementation                                                                                  |
| -------------------- | ----------------------------------------------------------------------------------------------- |
| **Playwright**       | `mask` option: array of Locators; covers with `#FF00FF` box                                     |
| **Cypress**          | `blackout` option: array of CSS selectors; covers with black box                                |
| **storycap-testrun** | `mask` parameter: covers elements with colored rectangles; `remove` parameter: deletes from DOM |
| **Lost Pixel**       | `mask` config: array of `{selector}` objects                                                    |
| **Chromatic**        | `ignoreSelectors`: excludes regions from diff comparison                                        |

**Status:** Not yet needed for snapvrt. Can be added later when users request it.

### 6. Per-Story Configuration

| Tool                 | Mechanism                                                                                |
| -------------------- | ---------------------------------------------------------------------------------------- |
| **Loki**             | `parameters.loki` in story metadata: `chromeSelector`, `skip`, viewport overrides        |
| **BackstopJS**       | Per-scenario: `readySelector`, `readyEvent`, `delay`, `hideSelectors`, `removeSelectors` |
| **Chromatic**        | `chromatic` parameter: `delay`, `pauseAnimationAtEnd`, `diffThreshold`                   |
| **Happo**            | `happo` parameter: `targets`, `delay`, themes                                            |
| **storycap-testrun** | `screenshotOptions` parameter: `mask`, `remove`, `delay`, `skip`                         |

**Status:** Future work for snapvrt. Not in scope for this iteration.

### 7. Concurrency & Performance

| Tool                 | Model                                                              |
| -------------------- | ------------------------------------------------------------------ |
| **snapvrt**          | N parallel tabs, round-robin job distribution                      |
| **Loki**             | N parallel tabs (default 4), batch processing                      |
| **Chromatic**        | Cloud parallelism (unlimited) + TurboSnap (skip unchanged stories) |
| **storycap-testrun** | Playwright browser contexts                                        |
| **BackstopJS**       | Sequential scenarios, viewports in parallel                        |

snapvrt's parallel tab model with round-robin distribution is solid.

---

## Recommendations for snapvrt (Ranked by Impact)

### 1. `captureBeyondViewport` — High Impact, Low Effort

**What:** Use CDP's native `captureBeyondViewport: true` parameter instead
of resizing the viewport for tall content.

**Why:**

- Eliminates 500ms resize delay per tall story
- No layout shifts from viewport resize
- No restore-viewport overhead
- Simpler code path

**Keep** the resize approach only for `--loki-compat` mode where pixel-perfect
parity with Loki is needed.

### 2. Screenshot Stability Check — High Impact, Medium Effort

**What:** After readiness detection, take up to N screenshots (default 3)
and compare consecutive pairs byte-for-byte. If two match, use that.

**Why:**

- Catches everything readiness heuristics miss: late font swaps, lazy images,
  JS-triggered reflows, CSS transition residue
- Used by Playwright, Chromatic, storycap-testrun — the most reliable tools
- Acts as a safety net rather than replacing existing readiness checks
- Byte-for-byte comparison is fast (no image decoding needed)

### 3. Web Animations API — Medium Impact, Low Effort

**What:** After page load, call `document.getAnimations().forEach(a => ...)`
to finish or cancel in-progress JS animations.

**Why:**

- CSS injection disables new CSS animations but doesn't touch JS animations
- framer-motion, GSAP, Lottie, and other JS animation libraries use the
  Web Animations API under the hood
- Playwright uses this exact approach
- Small JS injection, no new dependencies

### 4. Element Masking — Low Priority (Future)

**What:** `mask` config option (CSS selectors) that covers matched elements
with solid-color overlays before capture.

**Why:** Reduces false positives from dynamic content (timestamps, avatars).
Not needed until users request it.

### 5. Per-Story Configuration — Low Priority (Future)

**What:** Support `parameters.snapvrt` in story metadata for per-story
overrides (selector, delay, skip, mask).

**Why:** Useful for complex stories but requires Storybook integration.
Not needed until users request it.

---

## Implementation Status

| #   | Technique                  | Status      |
| --- | -------------------------- | ----------- |
| 1   | `captureBeyondViewport`    | Implemented |
| 2   | Screenshot stability check | Implemented |
| 3   | Web Animations API         | Implemented |
| 4   | Element masking            | Future      |
| 5   | Per-story configuration    | Future      |
