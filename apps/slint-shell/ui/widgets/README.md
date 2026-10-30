# Application Fluent widgets

This is a trimmed copy of the Fluent widgets shipped in `i-slint-compiler =1.18.1`
(`widgets/fluent`, `widgets/common`). Original copyright and SPDX headers are
retained; these files are used under their GPL-3.0-only option, compatible with
the application's GPL-3.0-or-later licence. This adds no Cargo dependency.

The public Slint palette does not expose Fluent's state, stroke and focus
colours. Keeping the palette here lets high contrast and E-ink apply to buttons,
checkboxes, combo boxes (including their popup), text editors, selection,
scrollbars and text context menus as well as application controls.

Local adaptations:

- `lhc/styling.slint`: explicit application accent, high contrast and E-ink
  palettes. State changes keep opaque, contrasting text and outlines.
- `lhc/color-scheme.slint`: the shell supplies the resolved system/light/dark
  choice through `Theme.apply`; no compiler-private `SlintInternal` references.
- `std-widgets*.slint`: only the widgets and compiler helpers used by the app.
  Radio choices use the application's `ButtonGroup`, including a vertical mode.
- `common/combobox-base.slint`: selection is driven by `current-index`;
  `current-value` is an output. Reverse lookup used a compiler-private API.
- `common/menus.slint`: flat text-editing context menus only; the application
  does not use a menubar or nested standard context menus. `MenuBarImpl` is
  present because the compiler requires that symbol for context-menu lowering.
- Removed compiler-private `@shadowable` and `@deprecated` annotations.

When upgrading Slint, compare these files with that release and reapply or remove
these adaptations. Do not enable experimental compiler features for this style.
