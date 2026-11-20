# Tri-Klops

Create images from triangles using a genetic algorithm.

![Tri-Klops application screenshot](https://raw.githubusercontent.com/kkestell/tri-klops/main/screenshot.png)

## Installation

Download the latest release from the [releases page](https://github.com/kkestell/tri-klops/releases) on GitHub.

## Build and run

Install Rust with [rustup](https://rustup.rs/), then run these commands from the checkout.

```sh
rustup update stable
cargo run --release --locked
```

Load a reference image and choose **Start Processing**. The preview shows the best candidate over the accepted triangles. When the run finishes, or when you stop it, Tri-Klops saves an SVG next to the reference image using the same filename stem. An existing SVG at that path is replaced.

### Shaping the triangles

Raise **Chunkiness** to rule out skinny triangles. Increase **Grid snap** to make corners land on shared grid points. At 0% chunkiness and 1 px spacing, the generator has full freedom. Choose **Start Processing** to use the new settings. [See these controls on The Great Wave](#the-great-wave-off-kanagawa).

## Package a release

Pushing a `vMAJOR.MINOR.PATCH` tag that matches the version in `Cargo.toml` builds the x64 Windows installer in GitHub Actions. Download it from the completed **Windows installer** run under **Artifacts**.

- On macOS, run `./publish-macos.sh` with the Xcode command-line tools installed to build the Intel and Apple Silicon app and zip archive.
- On x64 Windows, install Inno Setup 6 and run `.\install.ps1` in PowerShell to create the installer in `publish\`.

## Examples

All runs use seed 42. Deckard, Zardoz, and The Great Wave use 1024×1024 canvases and a 4,096-triangle budget.

### Deckard

![Triangle rendering of Deckard from Blade Runner](./examples/deckard-render.jpg)

[Still from *Blade Runner* (1982)](https://www.scenestill.com/storage/film-stills/webp/vlcsnap-2026-01-31-12h40m01s195_webp.webp) · [Reference crop](./examples/deckard-reference.jpg) · [512-triangle render at 256×256](./examples/deckard-512.jpg) · [Full-size SVG](./examples/deckard.svg)

### Zardoz

![Triangle rendering of Zardoz](./examples/zardoz-render.jpg)

[Reference from *Zardoz* (1974)](./examples/zardoz.jpg) · [512-triangle render at 256×256](./examples/zardoz-512.jpg) · [Full-size SVG](./examples/zardoz.svg)

### The Great Wave off Kanagawa

![Triangle rendering of Hokusai’s Great Wave using chunky triangles and a snapped grid](./examples/kanagawa-styled-render.jpg)

This render uses 90% **Chunkiness** and 16 px **Grid snap**.

[Reference print by Katsushika Hokusai](./examples/kanagawa.jpg) · [Free-placement render](./examples/kanagawa-render.jpg) · [Full-size SVG](./examples/kanagawa-styled.svg) · [Free-placement SVG](./examples/kanagawa.svg)

### J. Robert Oppenheimer

![Triangle rendering of J. Robert Oppenheimer holding a pipe in front of a chalkboard](./examples/oppenheimer-render.jpg)

The 1280×1635 render uses a 12,288-triangle budget.

[1958 portrait by Philippe Halsman](https://content.magnumphotos.com/wp-content/uploads/2019/05/cortex/par196080-teaser-story-big.jpg) · [Full-size SVG](./examples/oppenheimer.svg)

### Blade Runner in neon

![Triangle rendering of Deckard reading a newspaper against neon signs](./examples/blade-runner-neon-render.jpg)

The 1920×800 render uses free vertex placement and 320 search generations per triangle. The 16,384-triangle budget produced 12,955 triangles.

[Still from *Blade Runner* (1982)](https://www.scenestill.com/storage/film-stills/webp/vlcsnap-2026-01-31-12h33m14s004_webp.webp) · [Full-size SVG](./examples/blade-runner-neon.svg)
