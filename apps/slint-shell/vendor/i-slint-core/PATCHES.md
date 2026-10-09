# Local patches

Source: Slint 1.17.1, https://github.com/slint-ui/slint/tree/cf62c975c311e7036d599ed8ed0b7e6a8386a934/internal/core
The published crate's Cargo.toml and license files are preserved. The root Cargo.toml applies this copy through `[patch.crates-io]`.

- Record text replacements as one operation, retaining the selection before editing and the caret after editing.
- Discard redo on new edits and invalidate history when text is replaced externally.
- Group adjacent typing and deletion within one second; break groups at whitespace, navigation, focus changes, paste and composition commits.
- Restore the caret and selection for undo and redo; protect read-only inputs.
- Accept Ctrl+Y and Ctrl+Shift+Z for redo on both Linux and Windows, and uppercase shortcut letters for Caps Lock.
- Normalize text editing shortcuts in native input adapters: keep Latin layout shortcuts, use physical key positions for non-Latin layouts, exclude AltGr and other modified chords.

The history implementation is exercised by `slint-shell` unit tests through the same source file, and by `examples/undo-redo.rs` and `examples/inline-edit.rs` with real Slint widgets.
