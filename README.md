# Another Photo Slideshow Viewer

Another Photo Slideshow Viewer is a native Windows photo slideshow viewer for JPEG, PNG, and BMP. The windowed view has a dark photo canvas, a five-photo strip, and a playback bar. Fullscreen uses the current monitor and reveals playback controls on pointer movement or keyboard navigation.

## Build and run

Use 64-bit Windows 10 or 11, current stable Rust, and the Visual Studio C++ Build Tools with a Windows SDK. From this directory:

```powershell
cargo run --release
```

Pass a folder on the command line to open it immediately:

```powershell
cargo run --release -- "C:\Photos"
```

Use `cargo build --release` to produce `target\release\another-photo-slideshow-viewer.exe`.

## Controls

| Input | Action |
|---|---|
| Open folder or Ctrl+O | Open a folder in filename order |
| Left or Right | Previous or next photo |
| Space | Play or pause |
| Click a thumbnail | Jump to that photo |
| Settings | Choose the slide interval or toggle looping and shuffle |
| F11 / Esc | Enter or leave fullscreen |
| Tab / Shift+Tab / Enter | Move control focus and activate it |

The viewer starts paused and defaults to five seconds per photo with looping enabled. It applies photo orientation metadata, blends PNG transparency over the dark canvas, and reports unreadable images. JPEG photos from an iPhone can be opened directly. HEIC is outside this release's format list.

Enable **Shuffle slideshow** in Settings to play the current photo followed by
the other readable photos in random order. **Previous** retraces playback history.
With looping enabled, playback reshuffles after each pass. With looping off,
playback stops at the end of the pass.

## Folders and preferences

Drop a folder on the app window or use **Open folder**. You can also drop one
or more JPEG, PNG, or BMP files to start a paused slideshow of just those files,
in the order Windows supplies them. If you drop photos and folders together,
the viewer opens the selected photos. Photo drops do not change recent folders.

Settings lists the last
five folders and offers **Include subfolders**, which is off by default.
The viewer restores the last folder at startup unless you pass a folder on the
command line. Slide interval, looping, shuffle, and subfolder preferences are
saved for the current Windows user.

## Viewing controls

Click the display mode dropdown or press **M** to choose from **Fit**, **Fill**,
**Actual Size**, and **Stretch**. Fit shows the entire photo at its original
proportions. Fill crops to cover the canvas, Actual Size shows one image pixel
per screen pixel, and Stretch covers the canvas without preserving proportions.

Use the mouse wheel or **+** and **−** to zoom, drag the photo to pan, and
press **0** to reset. The mode stays selected when the slideshow advances.
Move the mouse or press Tab in fullscreen to reveal **Exit fullscreen**,
or press **Esc** at any time.

## Code

- `src/main.rs` contains window messages, controls, layout, drawing, and slideshow state.
- `src/photos.rs` contains folder discovery, background work, and the adapter for the open-source `image` crate.
- `src/win.rs` contains hand-written bindings to the Windows APIs used by the viewer.

The only direct third-party Cargo dependency is `image` (MIT or Apache-2.0), with only JPEG, PNG, and BMP enabled. All decoding is delegated to that crate. The application uses the Windows system APIs for the window, folder picker, and drawing.


## Versioning and releases

Cargo.toml defines the version displayed in the title bar.

GitHub Actions runs the Windows tests and release build on pushes to main and
development, and when a pull request is opened. After checks pass on a push to
main, the workflow publishes
a Windows x64 ZIP under the tag for the version in Cargo.toml. Existing releases
are skipped. For a new release, update the version in Cargo.toml and commit the
updated Cargo.lock.
