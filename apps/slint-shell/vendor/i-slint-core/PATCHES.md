# Local patches

Source: Slint 1.18.1, https://github.com/slint-ui/slint/tree/372cf0ee5577c3dfec309a45e7b778ba4e81b734/internal/core
The published crate's Cargo.toml and license files are preserved. Patched files keep upstream formatting (`rustfmt.toml` of the Slint repository: `use_small_heuristics = "Max"`), so the patch can be ported with a three-way merge. The root Cargo.toml applies this copy through `[patch.crates-io]`.

- Record text replacements as one operation, retaining the selection before editing and the caret after editing.
- Discard redo on new edits and invalidate history when text is replaced externally. Upstream 1.18 detects external `text` assignments in `align_to_text` (#331); that hook now resets this history instead of the upstream undo stacks.
- Group adjacent typing and deletion within one second; break groups at whitespace, navigation, focus changes, paste and composition commits.
- Restore the caret and selection for undo and redo; protect read-only inputs.
- Accept Ctrl+Y and Ctrl+Shift+Z for redo on both Linux and Windows, and uppercase shortcut letters for Caps Lock.
- Normalize text editing shortcuts in native input adapters: keep Latin layout shortcuts, use physical key positions for non-Latin layouts, exclude AltGr and other modified chords.

The history implementation is exercised by `slint-shell` unit tests through the same source file, and by `examples/undo-redo.rs` and `examples/inline-edit.rs` with real Slint widgets.

- Expose a reactive event-loop animation switch: disabled property transitions immediately reach their target, including standard Fluent widgets; animation-tick effects become static. The shell applies the accessibility preference in `ui::apply_theme`.

- Flickable wheel handling (`items/flickable.rs`): a wheel capture is released once the wheel has been idle for the filter duration or the pointer has moved, because winit never sends `Ended` for a mouse wheel whose first step is high-resolution (non-discrete); without this the capture lasted forever and the scroll view took every later wheel event. Upstream 1.18 already forwards a new wheel gesture (`TouchPhase::Started`) to the children first, so the former local patch for that was dropped.
