# Roadmap

## 0.4.0 — Viewing controls and UI (implemented)

- Refine the layout, spacing, icons, and keyboard focus cues in the windowed and fullscreen controls.
- Add a clearly labeled **Exit fullscreen** button to the fullscreen controls. Keep Esc available and reveal the button with pointer movement or keyboard navigation.
- Add **Fit**, **Fill**, **Actual Size**, and **Stretch** display modes. Keep the selected mode as the slideshow advances.
- Add manual zoom with the mouse wheel and keyboard, and pan by dragging. Reset manual zoom and pan when a new photo opens.

## 0.5.0 — Reliable playback

- Skip unreadable photos during manual navigation and summarize skipped files.
- Keep shuffle history limited to photos that appeared on screen.
- Add regression tests for rapid navigation and corrupt photos.

## 0.6.0 — Easier browsing

- Add a thumbnail grid with filename search and sorting by name or date modified.
- Start the slideshow from any photo selected in the grid.

## 0.7.0 — Saved slideshows

- Save and reopen named playlists that reference photos across multiple folders.
- Reorder playlist slides while keeping the original photos in place.

## 0.8.0 — Smoother fullscreen playback

- Add optional crossfades and custom slide durations.
- Add a fullscreen monitor picker.
- Keep manual keyboard navigation immediate when transitions are enabled.

## 0.9.0 — Release polish

- Improve keyboard and screen-reader access.
- Test large folders and mixed-DPI monitors.
- Add a Windows installer alongside the portable ZIP.
