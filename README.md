# Image Viewer

A fast, minimal, bezel-less image viewer written in Rust (egui/efui-free, built on
[eframe/egui](https://github.com/emilk/egui)). One window, your image, and nothing else.

![Language: Rust](https://img.shields.io/badge/language-Rust-orange)

## Features

- **Full-bleed main view** — the opened image, letterboxed, no borders or toolbars.
  Only normal OS window decorations.
- **Ambient background** — the current image also covers the entire window behind
  everything else, blurred and darkened with a black tint.
- **Hover navigation arrows** — previous/next chevrons fade in only while the pointer
  is over the main view.
- **Metadata side panel** — move the mouse into the rightmost 1/8th of the window and
  a translucent black panel slides in from the right over the image, its left edge
  softly faded into the picture (no hard border). It shows file name, location,
  format, dimensions, file size, last-modified time and selected EXIF fields (camera,
  lens, focal length, aperture, exposure, ISO, date taken, artist, software). It
  automatically hides after ~2 seconds of idling, or when the pointer leaves that
  zone.
- **Bottom carousel** — every other image in the same folder, with file names. Click a
  thumbnail (or use the arrow keys) to switch images; the selection is highlighted and
  the carousel scrolls to keep it in view. The carousel reserves its own layout space —
  it never covers the image.
- **Hideable carousel** — press `C` or click the small chevron button at the
  bottom-right of the view.
- **Drag & drop** — drop an image file (or folder) onto the window to open it.
- **Window title** shows the current file name.
- **Empty / error states** — a helpful hint when nothing is open, a readable message
  when a file can't be decoded.
- **Be a good citizen**: on Linux it never overwrites existing file associations, and
  on Windows everything lives under `HKEY_CURRENT_USER` (no admin rights needed).

### Supported formats

JPEG, PNG, GIF, BMP, WebP, TIFF, ICO, PNM — via the `image` crate, decoded off the UI
thread (thumbnails are cached with an LRU cap so large folders stay smooth).

## Keys

| Key           | Action                                        |
| ------------- | --------------------------------------------- |
| `←` / `→`     | Previous / next image in the current folder   |
| `C`           | Show / hide the bottom carousel               |
| mouse hover   | Nav arrows + metadata panel                   |
| drag & drop   | Open an image or folder                       |

## Install

### Linux

```sh
# build (needs a C/C++ toolchain and the usual X11/Wayland dev libraries
# via pkg-config: libxkbcommon, fontconfig/freetype)
cargo build --release
install -Dm755 target/release/image-viewer ~/.local/bin/image-viewer

# register as a default-app candidate (writes ~/.local/share/applications
# and merges into ~/.config/mimeapps.list without clobbering your defaults)
image-viewer --register
```

Uninstall with `image-viewer --unregister`, then delete the binary.

### Windows

1. Grab `image-viewer.exe` (see *Building* below) and put it anywhere, e.g.
   `C:\Program Files\Image Viewer\` or a portable folder.
2. Double-click **`install.bat`** (or run `image-viewer.exe --register`). This writes
   the registration under `HKEY_CURRENT_USER` — no admin prompt.
3. **Windows 10/11 note (honest one):** Microsoft no longer allows any app to silently
   become the *default* handler. After registering, pick "Image Viewer" once:
   - *Settings → Apps → Default apps* and set it for each image type, **or**
   - right-click any image → *Open with → Image Viewer* → tick **Always**.

Uninstall: double-click **`uninstall.bat`** (or run `image-viewer.exe --unregister`),
then delete the exe.

## Building

Requirements: Rust 1.75+ (`rustup` recommended).

```sh
# Linux / macOS
cargo build --release
# -> target/release/image-viewer

# Windows (native, MSVC or GNU toolchain)
cargo build --release --target x86_64-pc-windows-msvc   # on Windows
cargo build --release                                    # or plain

# Cross-compiling from Linux to Windows (mingw-w64):
#   apt install gcc-mingw-w64-x86-64-posix binutils-mingw-w64-x86-64
# then:
cargo build --release --target x86_64-pc-windows-gnu
# -> target/x86_64-pc-windows-gnu/release/image-viewer.exe
```

The release profile uses LTO, symbol stripping and `panic = "abort"`; the resulting
binary is a single ~6 MB executable with no runtime dependencies beyond the OS.

Repository layout: `src/main.rs` (CLI + registration entry points), `src/app.rs`
(UI, hover panel, carousel), `src/loader.rs` (background decode worker),
`src/meta.rs` (metadata + EXIF), `src/associate.rs` (file-manager registration).

## Usage

```sh
image-viewer                     # empty state, then drag & drop
image-viewer photo.jpg           # open one image
image-viewer ~/Pictures          # open a folder
image-viewer --register          # become a default-app candidate
image-viewer --unregister        # undo
image-viewer --help
```
