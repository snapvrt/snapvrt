# CI Integration

> TODO: Write CI integration guide

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
          node-version: '20'

      - name: Install dependencies
        run: npm ci

      - name: Build Storybook
        run: npm run build-storybook

      - name: Run visual tests
        run: npx snapvrt test --storybook-dir ./storybook-static

      - name: Upload diff artifacts
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: visual-diffs
          path: .snapvrt/snapshots/**/diff.png
```

## GitLab CI

> TODO

## CircleCI

> TODO
