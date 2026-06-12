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
  --title <title>            Set video_player window title
  --search-results <path>    Write search results as TSV and exit
  --thumbnail-batch <path>   Batch download thumbnails from manifest
  -h, --help                 Show help
```

### GUI (`yt-gui`)

```
yt-gui [QUERY]
```

Search, browse results, view details, and play videos with a graphical interface.

## Building

Requires the Scarlet Rust toolchain (see [Scarlet](https://github.com/petitstrawberry/Scarlet)).

```bash
cargo build --target aarch64-unknown-scarlet
```

Bundled into Scarlet via cargo-scarlet.

## Vendored Patches

- **`vendor/ring`**: Patched for `target_os = "scarlet"` (random source, LINUX_ABI, stack protector).

## License

MIT
