# UI principles

Rules for the Slint settings UI and popups. Check new UI against them.

## Colour

- **Destructive actions are not red.** Delete/Remove buttons and menu items use the normal
  text and icon colour. Confirmation dialogs already protect against mistakes; red is reserved
  for errors. Shared controls have no `danger` property on purpose — do not add one back.
- `Theme.error` is only for error messages, invalid values and failed states.
- Colours and metrics come from `theme.slint`; every theme (system, light, dark, E-ink,
  high contrast) must stay readable.

## Layout

- Buttons take the height of their content; a list of a few buttons must not stretch to fill
  free space.
- Small inline actions (clear, reveal) sit inside the field on the right, not as full-width
  buttons.
- Info icons next to a field are vertically centred with the field.

## Navigation

- A top-bar tab is highlighted only on its own page, not on nested detail pages.

## Text

- All visible text goes through `@tr(...)`; Rust sends `i18n::Msg`.
