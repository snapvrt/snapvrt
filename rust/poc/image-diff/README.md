# PoC 0.4 — Image Diff Engine Comparison

Compares three image diff engines side-by-side using a common `DiffEngine` trait:

| Engine  | Crate           | Approach                               |
| ------- | --------------- | -------------------------------------- |
| `dify`  | `dify`          | YIQ perceptual pixel diff + anti-alias |
| `ssim`  | `image-compare` | SSIM on luma + RMS on chroma/alpha     |
| `pixel` | (custom)        | Euclidean RGBA distance per pixel      |

## Usage

```bash
cargo run -p poc-image-diff -- --left <LEFT.png> --right <RIGHT.png> [--output <DIR>]
```

### Bundled test fixtures

The `fixtures/` directory contains images from the [dify](https://github.com/jihchi/dify) repo:

- `4a.png` — reference image
- `4b.png` — modified image (subtle differences)
- `4diff.png` — expected diff output from dify

Run with the bundled fixtures:

```bash
# Different images
cargo run -p poc-image-diff -- \
  --left  poc/image-diff/fixtures/4a.png \
  --right poc/image-diff/fixtures/4b.png \
  --output poc/image-diff/fixtures

# Identical images (should report score ≈ 0.0 for all engines)
cargo run -p poc-image-diff -- \
  --left  poc/image-diff/fixtures/4a.png \
  --right poc/image-diff/fixtures/4a.png \
  --output poc/image-diff/fixtures
```

### Output

Prints a comparison table and saves `diff-{engine}.png` to the output directory:

```
Engine          Score    Diff Pixels   Total Pixels  Time (ms)
--------------------------------------------------------------
dify         0.032296           5828         180456      43.94
ssim         0.166544          30054         180456     248.21
pixel        0.056673          10227         180456      12.68
```

Score convention: **0.0 = identical, 1.0 = completely different**.
