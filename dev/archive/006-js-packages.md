# JavaScript Packages Design

Status: Accepted | Date: 2026-02-08

> Part of [snapvrt specification](README.md)

This document defines the JavaScript/TypeScript packages for integrating snapvrt with test frameworks.

## Package Architecture

```
@snapvrt/client          Generic HTTP client for service API
├─ @snapvrt/jest    Jest matchers
└─ @snapvrt/vitest  Vitest matchers
```

| Package           | Purpose               |
| ----------------- | --------------------- |
| `@snapvrt/client` | Generic HTTP client   |
| `@snapvrt/jest`   | Jest async matchers   |
| `@snapvrt/vitest` | Vitest async matchers |

## `@snapvrt/client`

Generic client that works anywhere - Node.js scripts, test frameworks, CI tools.

### Installation

```bash
npm install @snapvrt/client
```

### Usage

```javascript
import { createClient } from "@snapvrt/client";

const client = createClient({ port: 7280 });

// Compare PDF against reference
const result = await client.comparePdf({
  name: "invoice",
  pdf: pdfBuffer,
  dpi: 144,
});
// { match: false, score: 0.0042 }

// Compare web page
const result = await client.compareWeb({
  name: "homepage",
  url: "http://localhost:3000/",
  viewport: { width: 1366, height: 768 },
});

// Approve a snapshot
await client.approve("invoice");

// Approve all pending
await client.approveAll();

// Get status
const status = await client.status();
// { pending: [{ name: 'invoice', status: 'failed' }] }
```

### API

```typescript
interface ClientOptions {
  port?: number; // Default: 7280
  host?: string; // Default: 'localhost'
}

interface ComparePdfOptions {
  name: string; // Snapshot name
  pdf: Buffer; // PDF content
  dpi?: number; // Default: 144
  pages?: string; // Default: 'all'
  merge?: boolean; // Default: false
}

interface CompareWebOptions {
  name: string; // Snapshot name
  url: string; // Page URL
  viewport?: {
    width: number;
    height: number;
  };
}

interface CompareResult {
  match: boolean;
  score: number; // 0 = identical
  isNew?: boolean; // True if no reference exists
}

interface Client {
  comparePdf(options: ComparePdfOptions): Promise<CompareResult>;
  compareWeb(options: CompareWebOptions): Promise<CompareResult>;
  approve(name: string): Promise<void>;
  approveAll(): Promise<void>;
  status(): Promise<Status>;
}
```

### Error Handling

```javascript
import { SnapvrtError, ServiceUnavailableError } from "@snapvrt/client";

try {
  await client.comparePdf({ name: "invoice", pdf });
} catch (error) {
  if (error instanceof ServiceUnavailableError) {
    console.error("Service not running. Start with: snapvrt service start");
  }
}
```

## `@snapvrt/jest`

Jest async matchers that wrap the client.

### Installation

```bash
npm install -D @snapvrt/jest
```

### Setup

```javascript
// jest.setup.js
import { toMatchPdfSnapshot, toMatchWebSnapshot } from "@snapvrt/jest";

expect.extend({ toMatchPdfSnapshot, toMatchWebSnapshot });
```

```javascript
// jest.config.js
module.exports = {
  setupFilesAfterEnv: ["./jest.setup.js"],
};
```

### Usage

```javascript
test("invoice renders correctly", async () => {
  const pdf = await generateInvoice();

  // Basic usage - name derived from test name
  await expect(pdf).toMatchPdfSnapshot();

  // Custom name
  await expect(pdf).toMatchPdfSnapshot("invoice-2024");

  // With options
  await expect(pdf).toMatchPdfSnapshot("invoice", { dpi: 72 });
});

test("homepage looks correct", async () => {
  await expect("http://localhost:3000/").toMatchWebSnapshot("homepage", {
    viewport: { width: 1366, height: 768 },
  });
});
```

### Snapshot Naming

When name is omitted, it's derived from the test name:

```javascript
describe("Invoice", () => {
  test("renders correctly", async () => {
    await expect(pdf).toMatchPdfSnapshot();
    // Snapshot name: "invoice-renders-correctly"
  });
});
```

### Matcher Behavior

| Scenario            | Result                          |
| ------------------- | ------------------------------- |
| Match               | Test passes                     |
| Mismatch            | Test fails with diff score      |
| New snapshot        | Test fails (must approve first) |
| Service unavailable | Test fails with helpful error   |

## `@snapvrt/vitest`

Vitest async matchers - identical API to Jest package.

### Installation

```bash
npm install -D @snapvrt/vitest
```

### Setup

```javascript
// vitest.setup.js
import { toMatchPdfSnapshot, toMatchWebSnapshot } from "@snapvrt/vitest";

expect.extend({ toMatchPdfSnapshot, toMatchWebSnapshot });
```

```javascript
// vitest.config.js
export default {
  test: {
    setupFiles: ["./vitest.setup.js"],
  },
};
```

### Usage

Same as Jest:

```javascript
test("invoice renders correctly", async () => {
  const pdf = await generateInvoice();
  await expect(pdf).toMatchPdfSnapshot();
});
```

## Workflow

```bash
# 1. Start service (once per session)
snapvrt service start

# 2. Run tests
npm test

# 3. Review failures
snapvrt review

# 4. Approve changes
snapvrt approve --all

# 5. Re-run tests (should pass)
npm test
```

## Configuration

Packages read from environment or config:

| Source                   | Priority    |
| ------------------------ | ----------- |
| `SNAPVRT_PORT` env var   | 1 (highest) |
| `createClient({ port })` | 2           |
| `.snapvrt/config.toml`   | 3           |
| Default (7280)           | 4 (lowest)  |

## Future Packages

| Package               | Purpose                | Status |
| --------------------- | ---------------------- | ------ |
| `@snapvrt/playwright` | Playwright integration | Future |
| `@snapvrt/cypress`    | Cypress integration    | Future |
