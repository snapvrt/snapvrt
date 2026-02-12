# CI Integration

## GitHub Actions

```yaml
name: Visual Tests

on: [push, pull_request]

jobs:
  visual-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: "20"

      - name: Install dependencies
        run: npm ci

      - name: Build Storybook
        run: npm run build-storybook

      - name: Start Storybook
        run: npx http-server storybook-static -p 6006 &

      - name: Wait for Storybook
        run: npx wait-on http://localhost:6006

      - name: Run visual tests
        run: snapvrt test

      - name: Upload report
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: snapvrt-report
          path: |
            .snapvrt/current/
            .snapvrt/difference/
```

**Notes:**

- Reference snapshots in `.snapvrt/reference/` must be committed to the repo
- The job exits with code 1 if any visual differences, new snapshots, or capture errors are found
- Upload the `current/` and `difference/` directories as artifacts for review

## Chrome Setup

snapvrt currently requires Chrome to be available. On CI, either:

1. **Use `--chrome-url`** to connect to a Chrome container you start yourself
2. **Use the default** which launches a local Chrome process (install Chrome in the CI image)

Docker-managed Chrome (auto-start/stop) is planned but not yet implemented.

### With Docker Chrome

```yaml
services:
  chrome:
    image: chromedp/headless-shell:latest
    ports:
      - 9222:9222

steps:
  # ...
  - name: Run visual tests
    run: snapvrt test --chrome-url http://localhost:9222
```

### With Local Chrome

```yaml
steps:
  - name: Install Chrome
    uses: browser-actions/setup-chrome@v1

  # ...
  - name: Run visual tests
    run: snapvrt test
```
