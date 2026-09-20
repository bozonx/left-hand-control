# Local prototype patches

Source: https://github.com/VimYoung/Spell/tree/c3139561519cbd78fc5ac8721e1b5a6eb6412f42/spell-framework
Version: 1.0.6. Original license is included in LICENSE.

This copy is used only by the independent Slint prototype.

- Commit unmap immediately and redraw after remapping without waiting for a stopped frame callback.
- Bound pending frame callbacks, avoid hidden-surface commits, and recreate compositor-closed layers after output removal.
- Deliver focus, committed rendered frame and layer closure through per-window callbacks.
- Deliver keyboard repeats and synchronize modifiers, including modifiers held before focus; release pressed keys on focus loss.
- Resolve output objects on each window's own Wayland connection.
- Calculate physical sizes from original logical sizes on every scale change; preserve logical layer geometry.

Frame events mean rendering and buffer commit, not compositor presentation.
