import { describe, expect, it } from "vitest";

import { createDefaultConfig } from "~/types/config";
import {
  applyPresetToConfig,
  builtinLayoutName,
  emptyLayoutPreset,
  extractPresetFromConfig,
  layoutSnapshotOf,
  localizeBuiltinLayoutPreset,
  parseLayoutYaml,
  serializeLayoutYaml,
} from "~/utils/layoutPresets";

describe("layout preset helpers", () => {
  it("parses yaml into normalized preset data", () => {
    const preset = parseLayoutYaml(`
name: Navigation
description: "  Primary nav layer  "
layers:
  - id: nav
    name: Nav
    keys:
      KeyH: ArrowLeft
      KeyJ: ~
      KeyL: ArrowRight
    extras:
      - name: MouseSide
        action: BrowserBack
rules:
  - key: CapsLock
    layer: nav
    tap: Escape
    hold: ~
    dtap: Ctrl+Alt+KeyT
    holdMs: 150
    dtapMs: 220
commands:
  - id: terminal
    name: Terminal
    linux: kitty
macros:
  - id: duplicateLine
    steps:
      - Ctrl+KeyC
      - keystroke: Ctrl+KeyV
        id: step-2
    stepPauseMs: 30
`);

    expect(preset).not.toBeNull();
    expect(preset).toMatchObject({
      description: "Primary nav layer",
      layers: [{ id: "nav", name: "Nav" }],
      rules: [
        {
          key: "CapsLock",
          layerId: "nav",
          tapAction: "Escape",
          holdAction: null,
          doubleTapAction: "Ctrl+Alt+KeyT",
          holdTimeoutMs: 150,
          doubleTapTimeoutMs: 220,
        },
      ],
      layerKeymaps: {
        nav: {
          keys: {
            KeyH: "ArrowLeft",
            KeyJ: null,
            KeyL: "ArrowRight",
          },
          extras: [
            {
              name: "MouseSide",
              action: "BrowserBack",
            },
          ],
        },
      },
      macros: [
        {
          id: "duplicateLine",
          name: "duplicateLine",
          stepPauseMs: 30,
          steps: [
            { keystroke: "Ctrl+KeyC" },
            { id: "step-2", keystroke: "Ctrl+KeyV" },
          ],
        },
      ],
      commands: [
        {
          id: "terminal",
          name: "Terminal",
          linux: "kitty",
        },
      ],
    });
  });

  it("preserves explicit text actions in layers, rules and macros", () => {
    const preset = parseLayoutYaml(`
layers:
  - id: sym
    keys:
      KeyA: "text:€"
rules:
  - key: AltRight
    tap: "text:."
macros:
  - id: snippet
    steps:
      - "text:TODO: "
`);

    expect(preset).not.toBeNull();
    expect(preset).toMatchObject({
      layerKeymaps: {
        sym: {
          keys: {
            KeyA: "text:€",
          },
        },
      },
      rules: [
        {
          key: "AltRight",
          tapAction: "text:.",
        },
      ],
      macros: [
        {
          id: "snippet",
          steps: [{ keystroke: "text:TODO: " }],
        },
      ],
    });

    const yaml = serializeLayoutYaml(preset!);
    expect(yaml).toContain('KeyA: text:€');
    expect(yaml).toContain('tap: text:.');
    expect(yaml).toContain("- 'text:TODO: '");
  });

  it("serializes and parses a preset without losing semantic values", () => {
    const original = {
      description: "editing helpers",
      layers: [{ id: "edit", name: "Edit", description: "editing layer" }],
      rules: [
        {
          id: "rule-1",
          key: "CapsLock",
          layerId: "edit",
          tapAction: "",
          holdAction: null,
          doubleTapAction: "Enter",
          holdTimeoutMs: 210,
          doubleTapTimeoutMs: undefined,
        },
      ],
      layerKeymaps: {
        edit: {
          keys: { KeyH: "ArrowLeft", KeyJ: null },
          extras: [{ id: "extra-1", name: "MouseSide", action: "BrowserBack" }],
        },
      },
      macros: [
        {
          id: "copyLine",
          name: "Copy line",
          steps: [
            { id: "s1", keystroke: "Home" },
            { id: "s2", keystroke: "Shift+End" },
          ],
          stepPauseMs: 10,
          modifierDelayMs: 5,
        },
      ],
      commands: [
        {
          id: "terminal",
          name: "Terminal",
          linux: "kitty",
        },
      ],
    };

    const yaml = serializeLayoutYaml(original);
    const reparsed = parseLayoutYaml(yaml);

    expect(reparsed).not.toBeNull();
    expect(reparsed).toMatchObject({
      description: "editing helpers",
      layers: original.layers,
      rules: [
        {
          key: "CapsLock",
          layerId: "edit",
          tapAction: "",
          holdAction: null,
          doubleTapAction: "Enter",
          holdTimeoutMs: 210,
          doubleTapTimeoutMs: undefined,
        },
      ],
      layerKeymaps: {
        edit: {
          keys: { KeyH: "ArrowLeft", KeyJ: null },
          extras: [{ name: "MouseSide", action: "BrowserBack" }],
        },
      },
      macros: [
        {
          id: "copyLine",
          name: "Copy line",
          steps: [{ keystroke: "Home" }, { keystroke: "Shift+End" }],
          stepPauseMs: 10,
          modifierDelayMs: 5,
        },
      ],
      commands: [
        {
          id: "terminal",
          name: "Terminal",
          linux: "kitty",
        },
      ],
    });
    expect(reparsed?.rules[0]?.id).toMatch(/^r_[a-z0-9]{8}$/);
  });

  it("extracts and applies presets while preserving settings and cloning layout data", () => {
    const config = createDefaultConfig();
    config.layers.push({ id: "nav", name: "Navigation" });
    config.rules.push({
      id: "rule-1",
      key: "CapsLock",
      layerId: "nav",
      tapAction: "",
      holdAction: "",
      doubleTapAction: "",
    });
    config.layerKeymaps.nav = {
      keys: { KeyH: "ArrowLeft" },
      extras: [{ id: "extra-1", name: "MouseSide", action: "BrowserBack" }],
    };
    config.macros.push({
      id: "duplicateLine",
      name: "Duplicate line",
      steps: [{ id: "step-1", keystroke: "Ctrl+KeyC" }],
    });
    config.commands.push({
      id: "terminal",
      name: "Terminal",
      linux: "kitty",
    });
    config.settings.appearance = "dark";

    const preset = extractPresetFromConfig(config);
    const next = applyPresetToConfig(
      createDefaultConfig(),
      preset,
      "user:current",
    );

    expect(next.settings.appearance).toBe("system");
    expect(next.settings.currentLayoutId).toBe("user:current");
    expect(next.layers).toEqual(config.layers);
    expect(next.rules).toEqual(config.rules);
    expect(next.layerKeymaps).toEqual(config.layerKeymaps);
    expect(next.macros).toEqual(config.macros);
    expect(next.commands).toEqual(config.commands);

    preset.layers[0]!.name = "Changed later";
    preset.layerKeymaps.nav!.keys.KeyH = "Changed";
    preset.commands[0]!.linux = "changed";
    expect(next.layers[0]!.name).toBe("Navigation");
    expect(next.layerKeymaps.nav!.keys.KeyH).toBe("ArrowLeft");
    expect(next.commands[0]!.linux).toBe("kitty");
  });

  it("creates stable layout snapshots and empty presets without implicit layers", () => {
    const config = createDefaultConfig();
    const initial = layoutSnapshotOf(config);

    config.rules.push({
      id: "rule-1",
      key: "CapsLock",
      layerId: "",
      tapAction: "Escape",
      holdAction: "",
      doubleTapAction: "",
    });

    expect(layoutSnapshotOf(config)).not.toBe(initial);
    expect(emptyLayoutPreset()).toEqual({
      layers: [],
      rules: [],
      layerKeymaps: {},
      macros: [],
      commands: [],
    });
  });

  it("parses and serializes isolate on a layer", () => {
    const preset = parseLayoutYaml(`
layers:
  - id: win
    isolate:
      - KeyW
    keys:
      KeyW: Ctrl+KeyA
`);
    expect(preset).not.toBeNull();
    expect(preset!.layerKeymaps.win).toMatchObject({
      keys: { KeyW: "Ctrl+KeyA" },
      isolate: ["KeyW"],
    });

    const yaml = serializeLayoutYaml(preset!);
    expect(yaml).toContain("isolate:");
    expect(yaml).toContain("- KeyW");
  });

  it("returns null for invalid or non-object yaml", () => {
    expect(parseLayoutYaml("not: [valid")).toBeNull();
    expect(parseLayoutYaml("hello")).toBeNull();
  });

  it("localizes the built-in preset from i18n without changing the user yaml format", () => {
    const preset = localizeBuiltinLayoutPreset(
      {
        description: "Fallback description",
        layers: [
          { id: "nav", name: "nav" },
          { id: "sel", name: "sel", description: "selection fallback" },
        ],
        rules: [],
        layerKeymaps: {},
        macros: [],
        commands: [],
      },
      (key) =>
        ({
          "builtinLayouts.ivank.name": "Localized built-in",
          "builtinLayouts.ivank.description": "Localized description",
          "builtinLayouts.ivank.layers.nav.name": "Navigation",
          "builtinLayouts.ivank.layers.nav.description": "Localized nav",
        })[key] ?? key,
    );

    expect(preset).toMatchObject({
      description: "Localized description",
      layers: [
        { id: "nav", name: "Navigation", description: "Localized nav" },
        { id: "sel", name: "sel", description: "selection fallback" },
      ],
    });
    expect(
      builtinLayoutName((key) =>
        key === "builtinLayouts.ivank.name" ? "Localized built-in" : key,
      ),
    ).toBe("Localized built-in");
  });
});
