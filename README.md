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

Load a reference image and choose **Start processing**. When the run finishes, or when you stop it, Tri-Klops saves an SVG next to the reference image using the same filename stem. An existing SVG at that path is replaced.

Portrait, landscape, and square images keep their framing in the preview and exported SVG.

### Browser build

Open [Tri-Klops in your browser](https://tri-klops.pages.dev). The app processes images locally in a Web Worker. Choose **Download SVG** or **Download JPG** after the run finishes or after you stop it. JPGs are rendered from the finished drawing at 2,048 pixels on the longest side. On a phone, open **Settings** to adjust the search parameters.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked
./web/build.sh
python3 -m http.server 8000 --bind 127.0.0.1 --directory target/web
```

Open [localhost:8000](http://localhost:8000). The `wasm-bindgen-cli` version must match `wasm-bindgen` in `Cargo.lock`.

To publish a build to the `tri-klops` Cloudflare Pages project, use [Wrangler Direct Upload](https://developers.cloudflare.com/pages/get-started/direct-upload/).

```sh
npx wrangler pages deploy target/web --project-name tri-klops --branch main
```

### Search modes

**One at a time** (default) searches for the next triangle to add. Each accepted triangle stays in place.

**Whole drawing** adds triangles while regularly revisiting their shapes and colors. Once the drawing reaches the **Triangle limit**, the search focuses on improving existing triangles. It refits colors based on the visible parts of each triangle and frees slots taken by fully hidden triangles. Use the **Generations** slider under **Triangle limit** to set the search budget for the entire drawing. Click either slider’s number to enter an exact value. The preview and saved SVG use the best drawing found, measured by mean squared RGB error.

Each mode keeps its own search settings while the app is open.

| Search mode | Generations | Population size | Selected | Mutation rate |
| --- | ---: | ---: | ---: | ---: |
| One at a time | 160 per triangle | 80 | 4 | 0.20 |
| Whole drawing | 2,048 per drawing | 64 | 4 | 0.15 |

### Shaping the triangles

Raise **Chunkiness** to rule out skinny triangles. Increase **Grid snap** to make corners land on shared grid points. At 0% chunkiness and 1 px spacing, the generator has full freedom. Choose **Start processing** to use the new settings. [See these controls on The Great Wave](#the-great-wave-off-kanagawa).

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
