# Native GUI and controller navigation

The GUI uses ScarletUI NavigationView.header, HeaderBar, TextField, Button,
IconView, Surface, GridView, and ScrollView. Normal uses a 28px search field with
14px text; Tablet uses 44px with 16px text. Video cards contain thumbnail, title,
and channel; selecting a card opens details with its Play video action.

![Normal desktop results](desktop-results.png)

![Video details with playback below its metadata](desktop-details.png)

![Tablet portrait results with compact rows and bottom navigation](tablet-results.png)

These are real Scarlet VM screenshots with live YouTube results, at density 2.
Search, continuation, thumbnails, details, and the existing yt/VideoPlayer stream
path are preserved. Existing system text input and the separate soft keyboard
provide search entry. Confirm/Cancel follow the configured gamepad menu mapping.

Eight host tests cover native RenderingPipeline key/TouchFrame handling,
cancellation, card/detail playback activation, held Confirm, selection/page
restoration, stale asynchronous responses, and title wrapping. The three screens
are rendered at six logical sizes, including 320×240 and portrait/landscape.
Set YT_QA_DIR to retain the rendered PPM fixtures. Run with --test-threads=1.

Release target builds passed. Live VM testing covered search → details → actual
YouTube H.264/AAC playback → pause/seek → return, and Normal/Tablet/Focused layouts.
VM key/pointer input was injected; physical gamepad/touch and end-to-end typing
through the separate soft keyboard remain unverified. Audio decode was observed,
but audible output and the GPU compositor's zero-copy path were not tested.

## Reproduce the host UI tests on macOS

Run from the parent directory so Cargo does not load the repository's
cross-target `build-std` configuration:

```sh
cargo test --manifest-path yt-for-scarlet/Cargo.toml -p yt-gui \
  --target aarch64-apple-darwin -- --test-threads=1
```

The pinned Scarlet runtime gates its native IPC trampoline to Scarlet targets;
no local dependency override is required. ScarletUI's `platform-sws` feature
is enabled only for the Scarlet target. On 2026-10-25, all eight host tests
and release builds of both `yt` and `yt-gui` for `aarch64-unknown-scarlet`
passed with ScarletUI `8efde207` and Scarlet `a88184df`.
