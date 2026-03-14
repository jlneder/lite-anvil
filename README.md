# Lite-Anvil

A lightweight text editor written in Lua with a Rust core.

Lite-Anvil is a fork of [Lite XL](https://github.com/lite-xl/lite-xl) that replaces the
original C backend with Rust.

## Purpose & Forking

This project exists partially as an experiment, and partially as something I just wanted for myself.

**No Support**

I do not intend to maintain or support this in any way, but wanted to share the code so anyone interested can freely use, learn from, or fork this project into something new.

There will be a tag "InitialPort" for the initial port into Rust, before I begin altering this further to suit my own ergonomics.

## Features

- Full Lua 5.4 plugin API — all Lite XL plugins work without modification
- SDL3 window and input handling
- FreeType2 font rendering with subpixel antialiasing
- PCRE2 regex engine
- Cross-platform filesystem monitoring via the `notify` crate
- No system Lua required — Lua 5.4 is vendored via `mlua`
- User `config.lua` as the main customization surface for fonts, theme colors, syntax colors, and UI tuning
- Multi-cursor editing, including select-next, select-all-occurrences, and find-to-multi-cursor workflows
- Remote SSH project editing via `sshfs`

## Editing Workflows

### Multi-cursor editing

Lite-Anvil supports "select many, edit once" workflows.

- `Ctrl+D` / `Cmd+D`: add the next occurrence of the current selection
- `Ctrl+Shift+L` / `Cmd+Shift+L`: select all occurrences of the current selection at once
- `Ctrl+Alt+L` / `Cmd+Option+L`: after `Ctrl+F` / `Cmd+F`, turn the current find term into multi-cursors for every match in the file

Typical flow:

1. Select a word or phrase, or run Find with `Ctrl+F` / `Cmd+F`.
2. Use `Ctrl+D` / `Cmd+D` to grow one match at a time, or `Ctrl+Shift+L` / `Cmd+Shift+L` to grab every occurrence of the current selection.
3. If you used Find, press `Ctrl+Alt+L` / `Cmd+Option+L` to convert all matches of the current find term into simultaneous selections.
4. Type once to edit all selected matches together.

### Remote SSH editing

Remote editing is implemented by mounting a remote path locally with `sshfs`, then opening that mount as a normal project.

Requirements:

- `sshfs` must be installed on the machine running Lite-Anvil
- SSH authentication should already work non-interactively, or be handled by your SSH agent

Usage:

1. Open the command palette.
2. Run `Remote Ssh Open Project` to replace the current project with a remote one, or `Remote Ssh Add Project` to add a second remote project.
3. Enter a remote spec in the form `user@host:/absolute/path`.
4. Browse and edit files normally in the tree view and editor.

The mount is cleaned up when the remote project is removed from the session.

Useful shortcuts:

- `Ctrl+P` runs the command palette.
- `Ctrl+Shift+O` opens a file from the current project.
- `Ctrl+Alt+O` opens a project folder.
- `Ctrl+T` shows document symbols through LSP when available.
- `Ctrl+Alt+T` shows workspace symbols through LSP when available.

## Building

### Dependencies

```
# Ubuntu / Debian
apt install libsdl3-dev libfreetype6-dev libpcre2-dev

# Fedora
dnf install SDL3-devel freetype-devel pcre2-devel

# Arch
pacman -S sdl3 freetype2 pcre2
```

Rust 1.85+ is required. Install via [rustup](https://rustup.rs).

**Note** You may need to build sdl3 yourself on some systems. I did on Linux Mint 22.2.

### Build

```
cargo build --release
```

The binary is placed at `target/release/lite-anvil`. See `BUILDING.md` for full
install and packaging instructions.

### Run

```
./target/release/lite-anvil [path]
```

## Install

```
make install          # installs to /usr/local by default
make install PREFIX=/usr
```

### macOS notes

If macOS reports `Code Signature Invalid` after install, re-sign the app bundle
locally with an ad hoc signature:

```bash
codesign --force --deep --sign - --timestamp=none /Applications/LiteAnvil.app
```

If the app was quarantined by Gatekeeper, remove the quarantine attribute:

```bash
sudo xattr -dr com.apple.quarantine /Applications/LiteAnvil.app
```

### Other Notes

Font: [Lilex](https://github.com/mishamyrt/Lilex)

## License

MIT — see [LICENSE](LICENSE).
