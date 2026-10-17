# FilmSweet app icon

**Mark:** a geometric film frame with a play triangle. A full-bleed rose tile holds a dark rounded
"film frame" whose centre is a light play mark — the same motif `crates/ui-egui/src/brand.rs` draws for
the in-app wordmark, so the icon and the app agree.

## Palette

Exactly three colours:

| Colour | Hex | Used for |
|---|---|---|
| FilmSweet rose (app colour) | `#ff4d8e` | the full-bleed field behind the frame |
| Ink | `#1b1c1f` | the film frame |
| Light | `#f4f4f6` | the play mark |

## Geometry and style

- A 512-unit viewBox tile, rounded square `rx=112`, with the app colour full bleed (no frame or border).
- A centred dark rounded square (`x=112 y=112 w=288 h=288 rx=64`) with a light right-pointing play
  triangle, so the mark stays legible from 16 px to 1024 px.
- macOS renders (`filmsweet.icns`, `filmsweet-macos-512.png`) put the tile in Apple's grid (824 of
  1024 px, transparent margin). Windows, Linux and the web use the full-bleed tile.

## Provenance

Original work made for FilmSweet by salim-slimani. The canonical artwork is the hand-written
`filmsweet.svg` in this directory; every other file here is derived from it. Licence:
[LICENSE.txt](LICENSE.txt) (MIT OR Apache-2.0).

## Files

| File | What |
|---|---|
| `filmsweet.svg` | canonical artwork (512-unit viewBox) |
| `filmsweet-small.svg` | the same mark as a compact file, for size-sensitive uses |
| `filmsweet-1024.png` | 1024 px render |
| `filmsweet-macos-512.png` | runtime Dock icon on macOS (embedded by `apps/filmsweet/src/main.rs`) |
| `filmsweet.icns` | macOS bundle icon (`CFBundleIconFile`) |
| `filmsweet.ico` | Windows icon, 16–256 px (embedded in `filmsweet.exe` by `apps/filmsweet/build.rs`) |
| `hicolor/<size>/apps/io.github.salimstudio.filmsweet.png`, `hicolor/scalable/…svg` | Linux icon theme, used by `packaging/linux/io.github.salimstudio.filmsweet.desktop` |

The web app's `apps/filmsweet-web/web/favicon.png` (128 px) comes from the same artwork.

## How the icon reaches each OS

- **macOS:** the window icon set at startup (`ViewportBuilder::with_icon`) is the Dock and app-switcher
  icon when the app runs unbundled. A future `.app` bundle points `CFBundleIconFile` at `filmsweet.icns`.
- **Windows:** `build.rs` embeds `filmsweet.ico` as the exe's resource icon (Explorer, Start menu,
  pinned taskbar), and the runtime icon covers the window, taskbar and Alt-Tab.
- **Linux:** the runtime icon plus `with_app_id("io.github.salimstudio.filmsweet")`, which Wayland matches to
  the `.desktop` file. Install the `hicolor/` tree under `/usr/share/icons/hicolor/` and the desktop
  file under `/usr/share/applications/`.

## Regenerate

`packaging/icons.sh` (needs `resvg`; `iconutil` on macOS for the `.icns`; the `.ico` is packed by
`cargo xtask ico`). To change the artwork, edit `filmsweet.svg` (and `filmsweet-small.svg`), then run the
script. Every derived file needs its `.attribution` sidecar and a row in `ATTRIBUTION.md`.
