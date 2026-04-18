---
title: Installation - Lite-Anvil
description: Install Lite-Anvil from prebuilt binaries or build from source on Linux, macOS, and Windows. Requires Rust 1.85+, SDL3, FreeType2, PCRE2.
---

# Installation

## Prebuilt Binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/danpozmanter/lite-anvil/releases).

Release archives contain both the `lite-anvil` (full editor) and `nano-anvil` (minimal single-file editor) binaries. Install whichever you need, or both.

### Linux

Pick whichever format matches your distro:

- **Debian / Ubuntu**: download `lite-anvil_*.deb` (and optionally `nano-anvil_*.deb`) and install with:
  ```bash
  sudo apt install ./lite-anvil_*_amd64.deb
  ```
- **Fedora / RHEL / openSUSE**: download `lite-anvil-*.rpm` and install with:
  ```bash
  sudo dnf install ./lite-anvil-*.x86_64.rpm
  ```
- **Anywhere else (Arch, NixOS, Gentoo, ...)**: download `lite-anvil-*-x86_64.AppImage`, make it executable, and run it:
  ```bash
  chmod +x lite-anvil-*-x86_64.AppImage
  ./lite-anvil-*-x86_64.AppImage
  ```
- **Manual / portable**: extract `lite-anvil-*-linux-x86_64.tar.gz` and copy the binary + `data/` directory to wherever you like. Desktop entry + icon are included in `resources/linux/` and `resources/icons/`.

### macOS

Download `lite-anvil-*-macos-{x86_64,aarch64}.dmg` for your architecture. Double-click the `.dmg`, then drag `LiteAnvil.app` and `NanoAnvil.app` onto the `Applications` shortcut. Launch from Launchpad or Spotlight.

Because the build is ad-hoc signed (not paid-notarized), Gatekeeper will warn on first launch. Right-click the app and choose *Open* once; subsequent launches go through without prompting.

### Windows

Download `LiteAnvil-*-x86_64-setup.exe` and run it. The installer bundles both `lite-anvil.exe` and `nano-anvil.exe`, creates Start Menu shortcuts, and offers optional file-association and *Add to PATH* tasks. A SmartScreen warning appears the first time (the build is unsigned) — click *More info* → *Run anyway*.

## Building from Source

### Requirements

- **Rust 1.85+** via [rustup](https://rustup.rs)
- System libraries:

| Library | Ubuntu/Debian | Fedora | Arch | macOS (Homebrew) |
|---------|--------------|--------|------|------------------|
| SDL3 | `libsdl3-dev` | `SDL3-devel` | `sdl3` | `sdl3` |
| FreeType2 | `libfreetype6-dev` | `freetype-devel` | `freetype2` | `freetype` |
| PCRE2 | `libpcre2-dev` | `pcre2-devel` | `pcre2` | `pcre2` |

### Build & Run

```bash
git clone https://github.com/danpozmanter/lite-anvil.git
cd lite-anvil
cargo build --release
./target/release/lite-anvil [path]
```

### macOS App Bundle (from source)

```bash
mkdir -p LiteAnvil.app/Contents/MacOS
cp target/release/lite-anvil LiteAnvil.app/Contents/MacOS/
cp -r data LiteAnvil.app/Contents/MacOS/
cp resources/macos/Info.plist LiteAnvil.app/Contents/
codesign --force --deep --sign - --timestamp=none LiteAnvil.app
```

### Debian Package

```bash
cargo install cargo-deb
cargo deb --no-build
```

## Configuration

User config location:

| Platform | Path |
|----------|------|
| Linux | `~/.config/lite-anvil/` |
| macOS | `~/Library/Application Support/lite-anvil/` |
| Windows | `%APPDATA%\lite-anvil\` |

Key files:

- `config.toml` -- editor settings (see [Configuration](guide.md#configuration))
