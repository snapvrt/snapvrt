# Multi-Source CLI Design

Status: Accepted | Date: 2026-02-08

> Part of [snapvrt specification](README.md)

This document defines the CLI design for supporting multiple screenshot sources (Storybook, PDF, web pages, Figma).

## Design Goals

1. **Simple for single-source projects** - `snapvrt test` just works
2. **Explicit for multi-source projects** - Clear source selection
3. **Consistent commands across sources** - Same verbs where semantics align
4. **Source-specific options** - Each source has unique configuration
5. **Unified review workflow** - All sources share approval/review flow

## CLI Reference

See [CLI Reference](../docs/cli-reference.md) for full command documentation.

## Source Types

| Source      | Discovery         | Modes                 | Status |
| ----------- | ----------------- | --------------------- | ------ |
| `storybook` | Auto (index.json) | Batch (CLI)           | v1     |
| `pdf`       | Service API       | Service (Jest/Vitest) | v1.1   |
| `pdf`       | Manifest          | Batch (CLI)           | Future |
| `web`       | Manifest          | Batch (CLI)           | Future |
| `figma`     | API               | Batch (CLI)           | Future |

**Key insight:** Sources have different primary workflows:

- **Batch** - Define what to test in config/manifest, run all at once via CLI
- **Service** - Call from test code (Jest/Vitest) via HTTP API

See [006-js-packages.md](006-js-packages.md) for JavaScript client and test framework integration.

## Configuration

See [Configuration Reference](../docs/configuration.md) for config file format, all options, and override precedence.

## Manifest Files

### PDF Manifest (`snapvrt-pdfs.json`)

```json
{
  "pdfs": [
    {
      "name": "invoice",
      "path": "./fixtures/invoice.pdf"
    },
    {
      "name": "report",
      "path": "./fixtures/report.pdf",
      "dpi": 72,
      "pages": "1-3"
    },
    {
      "name": "remote-doc",
      "url": "https://example.com/doc.pdf",
      "merge": false
    }
  ]
}
```

| Field   | Required | Default | Description                      |
| ------- | -------- | ------- | -------------------------------- |
| `name`  | Yes      | -       | Snapshot identifier              |
| `path`  | \*       | -       | Local file path                  |
| `url`   | \*       | -       | Remote URL (alternative to path) |
| `dpi`   | No       | 144     | Render resolution                |
| `pages` | No       | "all"   | Page range ("1", "1-3", "all")   |
| `merge` | No       | false   | Stack pages into single image    |

\*One of `path` or `url` required.

### Web Manifest (`snapvrt-pages.json`)

```json
{
  "pages": [
    {
      "name": "home",
      "url": "http://localhost:3000/"
    },
    {
      "name": "about",
      "url": "http://localhost:3000/about",
      "viewport": "mobile"
    },
    {
      "name": "dashboard",
      "url": "http://localhost:3000/dashboard",
      "wait_for": "#main-content"
    }
  ]
}
```

## Local File Access

Manifests can reference local files (`path` field), but Docker containers can't access host paths directly. The orchestrator serves local files via HTTP.

**How it works:**

1. Manifest specifies local path: `"path": "./fixtures/invoice.pdf"`
2. Orchestrator resolves path relative to project root (where `.snapvrt/` lives)
3. Files served via `/files/*` endpoint
4. Container fetches `http://host.docker.internal:{port}/files/fixtures/invoice.pdf`

**Service mode:** Uses the already-running Axum server.

**Standalone mode:** Spins up a temporary HTTP server before launching containers, stops it after completion.

**Security:**

- Only files under project root are served
- Path traversal (`..`) is rejected
- Symlinks are not followed outside project root

**Why not volume mounts:**

- Docker path mapping is platform-specific (Docker Desktop, Colima, Linux native)
- Host paths differ across OS (`/Users`, `/home`, Windows)
- HTTP approach works uniformly regardless of Docker configuration

## Source Auto-Detection

When `[SOURCE]` is omitted, the CLI determines which sources to run:

```
1. Read config.toml
2. For each source in [sources.*]:
   a. Validate required fields exist
   b. If missing required fields → hard error (don't skip silently)
   c. If valid → include
3. If no sources configured → error with helpful message
4. If one source → run it
5. If multiple sources → run all
```

### Required Fields

| Source    | Required Fields       |
| --------- | --------------------- |
| storybook | `url` OR `static_dir` |
| pdf       | `manifest`            |
| web       | `manifest`            |

### Error on Incomplete Config

If a source section exists but is incomplete, fail with a clear error:

```sh
$ snapvrt test
Error: [sources.storybook] is missing required field.

Provide one of:
  url = "http://localhost:6006"    # Dev server
  static_dir = "./storybook-static" # Static build
```

### No Sources Configured

```sh
$ snapvrt test
Error: No sources configured.

Add a source to .snapvrt/config.toml:

  [sources.storybook]
  url = "http://localhost:6006"

Or run: snapvrt init storybook
```

## Multi-Source Output

When testing multiple sources, output is grouped:

```sh
$ snapvrt test

── Storybook ──────────────────────────────────────────────
  ✓ Button/Primary (desktop)
  ✓ Button/Primary (mobile)
  ⊕ Button/Danger (desktop)
  ✗ Card/Default (desktop)

  4 stories: 2 passed, 1 new, 1 failed

── PDF ────────────────────────────────────────────────────
  ✓ invoice
  ✓ report

  2 documents: 2 passed

── Summary ────────────────────────────────────────────────
  Total: 6 snapshots, 2 passed, 1 new, 1 failed
  Run 'snapvrt review' to inspect changes.
```

## Multi-Source Error Handling

| Error type                                      | Behavior                                   | Exit code |
| ----------------------------------------------- | ------------------------------------------ | --------- |
| Config error (bad TOML, missing manifest)       | Fail fast before running anything          | 2         |
| Source error (URL unreachable, container crash) | Continue with other sources, report at end | 2         |
| Test failures (diffs detected)                  | Continue, report                           | 1         |

**Exit code precedence:** `2` (error) > `1` (failures) > `0` (pass)

## New Snapshot Handling

When a new story/page/document is discovered that has no reference snapshot:

### Behavior

New snapshots are treated as **requiring approval**, not as automatic passes or failures.

| Status  | Symbol | Meaning                                 |
| ------- | ------ | --------------------------------------- |
| Passed  | `✓`    | Reference exists, current matches       |
| Failed  | `✗`    | Reference exists, current differs       |
| New     | `⊕`    | No reference exists, needs approval     |
| Removed | `⊖`    | Reference exists, not in current source |

### Exit Codes

New snapshots exit with code 1 because:

- CI should fail until new snapshots are explicitly approved
- Prevents accidental baseline additions
- Forces review of what's being added

### Auto-Add Mode (Development Convenience)

For local development, auto-adding new snapshots can be enabled:

```sh
snapvrt test --auto-add-new
```

When enabled:

- New snapshots are automatically saved as references
- Exit code is 0 if only new snapshots (no failures)
- Warning is printed: `Auto-added 3 new snapshots`

**Recommendation:** Never enable `auto_add_new` in CI.

### Removed Snapshots

When a story/page is removed from the source but reference still exists:

| Option          | Behavior                               |
| --------------- | -------------------------------------- |
| Default         | Warn but don't delete (safe)           |
| `--prune`       | Delete orphaned references during test |
| `snapvrt prune` | Separate command to clean up orphans   |

## Service API

> **Internal API.** Use the official clients (`@snapvrt/client`, `@snapvrt/jest`, `@snapvrt/vitest`) which will be kept compatible. See [006-js-packages.md](006-js-packages.md).

| Endpoint            | Method | Description                        |
| ------------------- | ------ | ---------------------------------- |
| `/health`           | GET    | Health check                       |
| `/files/*`          | GET    | Serve local files to containers    |
| `/status`           | GET    | Pending diffs, operations          |
| `/status?source=X`  | GET    | Pending diffs filtered by source   |
| `/shutdown`         | POST   | Graceful shutdown                  |
| `/storybook/test`   | POST   | Run Storybook batch                |
| `/storybook/update` | POST   | Update Storybook refs              |
| `/pdf/test`         | POST   | Run PDF batch (manifest)           |
| `/pdf/update`       | POST   | Update PDF refs                    |
| `/pdf/compare`      | POST   | Compare single PDF                 |
| `/web/compare`      | POST   | Compare single web page            |
| `/approve`          | POST   | Approve snapshot                   |
| `/approve-all`      | POST   | Approve all pending                |
| `/review`           | GET    | Serve review UI                    |
| `/review?source=X`  | GET    | Serve review UI filtered by source |
| `/ws`               | WS     | Live updates                       |

**CLI → API mapping:**

- `snapvrt review pdf` opens browser to `/review?source=pdf`
- `snapvrt service status` calls `/status`

## Snapshot Naming

Snapshots are stored with source prefix for clarity:

```sh
.snapvrt/snapshots/
├── storybook/
│   ├── button-primary-desktop/
│   │   ├── reference.png
│   │   └── current.png
│   └── card-default-mobile/
│       └── ...
├── pdf/
│   ├── invoice/
│   │   ├── reference.png
│   │   └── current.png
│   └── report/
│       └── ...
└── web/
    └── home-desktop/
        └── ...
```
