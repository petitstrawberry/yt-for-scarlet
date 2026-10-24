# yt-for-scarlet

YouTube client for [Scarlet](https://github.com/petitstrawberry/Scarlet) OS.

## Crates

| Crate | Description |
|---|---|
| `scarlet-youtube` | YouTube data types and TSV serialization |
| `scarlet-youtube-net` | HTTP/TLS client, YouTube API, media streaming |
| `yt` | Terminal YouTube client |
| `yt-gui` | GUI YouTube client (scarlet-ui) |

## Prerequisites

- Rust nightly with `aarch64-unknown-scarlet` and/or `riscv64gc-unknown-scarlet` targets
- [cargo-scarlet](https://github.com/petitstrawberry/Scarlet) for building and running on Scarlet

## Usage

### CLI (`yt`)

```
yt [options] URL
yt [options] search QUERY
yt [options] QUERY

Options:
  -o, --output <path>        Save response body
  --headers                  Print response headers
  --no-play                  Download only
  --loop                     Loop playback
  --title <title>            Set video-player window title
  --search-results <path>    Write search results as TSV and exit
  --thumbnail-batch <path>   Batch download thumbnails from manifest
  -h, --help                 Show help
```

### GUI (`yt-gui`)

```
yt-gui [QUERY]
```

Search, browse results, view details, and play videos with a graphical interface.

The GUI uses ScarletUI's native navigation, header, text field, and buttons.
Normal posture uses compact controls; Tablet posture increases touch targets and
search text size. Results adapt to the available content area, and playback is
available in the selected video's details.

- Gamepad menu directions move between results and header controls. Confirm
  opens details or activates the focused control; Cancel returns to results or
  opens/cancels search. The system's Confirm/Cancel mapping is respected.
- Search uses the system text input and IME, including its separate soft keyboard.
- In VideoPlayer, Confirm toggles playback, Left/Right seek five seconds, and
  Up/Down move between playback, loop, seek, and fullscreen controls. The lower
  right fullscreen button or F11 toggles fullscreen. Cancel first leaves
  fullscreen; outside fullscreen it closes playback and restores the selected
  result. Keyboard Tab also moves between controls.
- Touch/pointer controls support playback, loop, and drag seeking. Cancelling a
  drag or resizing the window during a drag keeps the previously committed time.

[Screenshots and validation](docs/controller-ui.md)

## Building

Requires the Scarlet Rust toolchain (see [Scarlet](https://github.com/petitstrawberry/Scarlet)).

```bash
cargo build --target aarch64-unknown-scarlet
```

Bundled into Scarlet via cargo-scarlet.

The ScarletUI dependency is pinned to a revision containing the resize redraw
fix. No neighboring ScarletUI checkout is required. Use a Scarlet build with the
corresponding VideoPlayer controls and fullscreen support.

## Vendored Patches

- **`vendor/ring`**: Patched for `target_os = "scarlet"` (random source, LINUX_ABI, stack protector).

## License

MIT
