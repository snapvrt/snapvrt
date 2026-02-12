# Wire Protocols

Status: Accepted | Date: 2026-02-08

> Part of [snapvrt specification](README.md)

This is the single source of truth for all HTTP protocols between snapvrt components.

## Health Check

All workers (capture and diff) respond to:

```
GET /health → { "version": "0.1.0", "protocol": 1 }
```

CLI checks compatibility on pool startup and errors if mismatched.

**Open concern — versioning strategy:** The health response includes `protocol: 1` but there's no defined upgrade path. When the protocol changes: will CLI and worker images need to be version-locked? How will snapshot format changes (e.g., manifest.json schema) be handled? Define during Phase 1 when the wire crate is built.

## Story Discovery

```
GET http://localhost:6006/index.json
```

Response:

```json
{
  "v": 5,
  "entries": {
    "example-button--primary": {
      "type": "story",
      "id": "example-button--primary",
      "name": "Primary",
      "title": "Example/Button",
      "tags": ["dev", "test"],
      "importPath": "./src/components/Button.stories.tsx"
    }
  }
}
```

Filtering:

- Only `type: "story"` (exclude docs)
- Skip stories with `snapvrt-skip` tag

## Capture Protocol

Two endpoints for web and PDF content.

### Web Screenshot

```
POST /screenshot/web
Content-Type: application/json

{
  "url": "http://host.docker.internal:6006/iframe.html?id=button--primary",
  "viewport": { "width": 1366, "height": 768 },
  "deviceScaleFactor": 1
}
```

| Field               | Description                                    |
| ------------------- | ---------------------------------------------- |
| `url`               | Web page URL                                   |
| `viewport.width`    | Viewport width in CSS pixels                   |
| `viewport.height`   | Viewport height in CSS pixels                  |
| `deviceScaleFactor` | Pixel density ratio (1 = standard, 2 = retina) |

### PDF Screenshot

```
POST /screenshot/pdf
Content-Type: application/json

{
  "url": "http://host.docker.internal:9999/invoice.pdf",
  "dpi": 144,
  "pages": "all",
  "merge": false
}
```

| Field   | Description                                                           |
| ------- | --------------------------------------------------------------------- |
| `url`   | PDF URL                                                               |
| `dpi`   | Resolution (72 = low, 144 = high, default: 144)                       |
| `pages` | `"all"`, `"1"`, `"1-3"`, `"1,3,5"` (default: all)                     |
| `merge` | `true`: single PNG (stacked), `false`: per-page PNGs (default: false) |

### Response (single image)

Used for web screenshots and `merge: true` PDFs.

```
HTTP 200 OK
Content-Type: image/png
X-Timing-Navigate: <ms>
X-Timing-Render: <ms>
X-Timing-Screenshot: <ms>
X-Pages: <count>

<raw PNG bytes>
```

### Response (multi-page PDF, `merge: false`)

Multipart response with one part per page:

```
HTTP 200 OK
Content-Type: multipart/mixed; boundary=snapvrt

--snapvrt
X-Page: 1
X-Timing-Render: 12
Content-Type: image/png

<page 1 PNG bytes>

--snapvrt
X-Page: 2
X-Timing-Render: 8
Content-Type: image/png

<page 2 PNG bytes>

--snapvrt--
```

Why multipart: worker opens/parses the PDF once (not N times), pages stream as they render, and each page enters the diff pipeline as it arrives.

### Error Response

```
HTTP 500 Internal Server Error
Content-Type: text/plain

<error message>
```

### URL Resolution

CLI derives the container-accessible URL from `storybook_url` config:

| Host Config                 | Container URL                                      |
| --------------------------- | -------------------------------------------------- |
| `http://localhost:6006`     | `http://host.docker.internal:6006/iframe.html?...` |
| `http://127.0.0.1:6006`     | `http://host.docker.internal:6006/iframe.html?...` |
| `http://my-server.com:6006` | `http://my-server.com:6006/iframe.html?...`        |

**Linux note:** `host.docker.internal` requires Docker 20.10+ with `--add-host=host.docker.internal:host-gateway`. The CLI adds this flag automatically when spawning containers.

### Ready Detection

Wait until all conditions are met (with 10s timeout):

1. Network idle (no pending requests for 500ms, ignoring WebSocket/EventSource)
2. Fonts loaded (`document.fonts.ready`)
3. DOM stable (no mutations for 100ms)

**Note:** Long-lived connections (HMR websocket, polling) are excluded from network idle detection to prevent hangs.

### Injected Styles

```css
/* Disable CSS animations/transitions */
*,
*::before,
*::after {
  transition: none !important;
  animation: none !important;
}

/* Disable pointer events (prevent hover states) */
* {
  pointer-events: none !important;
}

/* Hide input carets */
* {
  caret-color: transparent !important;
}
```

### Anti-Flake

Visual regression tests are notoriously flaky. The spec covers animation disabling and ready detection but doesn't yet address:

- Retry-on-failure for flaky screenshots
- Threshold tuning guidance for users
- Known non-determinism sources (sub-pixel rendering, GPU differences)
- Baseline update strategy when Chrome is upgraded in the container image

Address during Phase 1 (capture implementation) and Phase 2 (compare pipeline).

### Font Consistency

Screenshots depend on font rendering. The Dockerfile installs `fonts-liberation` and `fonts-noto-color-emoji`, but:

- No custom font support (user apps using Inter, Roboto, etc.)
- `document.fonts.ready` may not cover all font loading scenarios
- Chrome version upgrades can change font rendering

Address during Phase 0.2 (CDP screenshot PoC) — test with custom fonts.

### Screenshot Cropping

Crop to `<body>` bounding box, not full viewport. This handles varying component sizes.

### Chrome DevTools Protocol Flow

1. Launch Chrome with `--headless --disable-gpu --hide-scrollbars --no-sandbox`
2. Connect via CDP (Chrome DevTools Protocol)
3. For each story:
   a. Create new tab
   b. Inject helper scripts (disable animations, pointer events)
   c. Navigate to story URL
   d. Wait for UI ready (see Ready Detection above)
   e. Get `<body>` bounding box
   f. Capture screenshot cropped to content
   g. Close tab

## Diff Protocol

### Request

```
POST /diff
Content-Type: multipart/form-data; boundary=...

--boundary
Content-Disposition: form-data; name="reference"; filename="reference.png"
Content-Type: image/png

<raw PNG bytes>

--boundary
Content-Disposition: form-data; name="current"; filename="current.png"
Content-Type: image/png

<raw PNG bytes>

--boundary
Content-Disposition: form-data; name="threshold"

0.0001
--boundary--
```

### Response

Metadata in headers, optional diff PNG in body:

```
# Match (no body)
HTTP 200 OK
X-Match: true
X-Score: 0.0
X-Engine: dify
Content-Length: 0

# Mismatch (diff PNG in body)
HTTP 200 OK
X-Match: false
X-Score: 0.00042
X-Engine: dify
Content-Type: image/png

<raw diff PNG bytes>
```

| Header     | Description                                    |
| ---------- | ---------------------------------------------- |
| `X-Match`  | `true` if score <= threshold                   |
| `X-Score`  | Difference score (0 = identical)               |
| `X-Engine` | Engine name (for display context)              |
| Body       | Diff PNG bytes (only present if match = false) |

### Score Semantics

Scores are engine-specific. No normalization — different engines have fundamentally different scales (pixel counts vs perceptual distance).

Threshold config is per-engine with recommended defaults:

```toml
# dify: fraction of differing pixels (0.0-1.0)
threshold = 0.001

# dssim: perceptual distance (0 = identical, unbounded)
# threshold = 0.01
```

## Header Constants

Defined in `snapvrt-wire/protocol.rs`:

| Constant        | Value      |
| --------------- | ---------- |
| `HEADER_MATCH`  | `X-Match`  |
| `HEADER_SCORE`  | `X-Score`  |
| `HEADER_ENGINE` | `X-Engine` |
| `HEADER_PAGE`   | `X-Page`   |
