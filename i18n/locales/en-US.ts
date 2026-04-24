// English (en-US) locale messages.
export default {
  app: {
    title: "Left Hand Control",
    badge: "Linux key-mapper",
    loading: "loading…",
    saving: "saving",
    saved: "saved",
    saveFailedTitle: "Failed to save changes",
    loadFailedBody:
      "The app could not be initialized. Check the error below and try loading the config again.",
    notSavedBadge: "• unsaved",
    dirtyTooltip:
      "The current layout has unsaved changes. Save it under My layouts, otherwise they will be lost when switching.",
    customLayout: "Custom layout",
    switchToLight: "Switch to light theme",
    switchToDark: "Switch to dark theme",
    presetLabel: "Preset:",
    layoutLanguageLabel: "Language:",
  },
  common: {
    cancel: "Cancel",
    save: "Save",
    delete: "Delete",
    apply: "Apply",
    create: "Create",
    clear: "Clear",
    edit: "Edit",
    confirm: "Confirm",
    refresh: "Refresh",
    retry: "Retry",
    add: "Add",
    close: "Close",
    copy: "Copy",
    copied: "Copied",
    reset: "Reset to default",
    override: "Override",
    ms: "ms",
    active: "active",
    stopped: "stopped",
    none: "— none —",
    asInSystem: "Use system",
    backToTop: "back to top",
  },
  tabs: {
    layouts: "My layouts",
    rules: "Rules",
    keymap: "Layers",
    macros: "Macros",
    commands: "Commands",
    settings: "Settings",
  },
  welcome: {
    intro:
      "Welcome! Pick a starting layout — you can change it anytime in Settings.",
    recommended: "recommended",
    useThis: "Use this layout",
    emptyTitle: "Empty layout",
    emptyDesc:
      "Start from scratch: no layers, rules or macros. Good if you want to build your own stack of additional layers from zero.",
    emptyBtn: "Start empty",
    footnote: "You can always change the chosen layout under {path}.",
    footnotePath: "Settings → Layouts",
    loadError: "Failed to load the built-in layout.",
  },
  builtinLayouts: {
    ivank: {
      name: "Ivan K's left hand control",
      description:
        "Pre-made author's layout: CapsLock — navigation layer, right Alt — symbols layer, left Alt — window manager. Includes a set of IDE macros.",
      layers: {
        nav: {
          name: "Navigation",
          description:
            "Navigation through text and code: arrows, Home/End, PageUp/Down, and movement by words and lines. Activated while CapsLock is held.",
        },
        sel: {
          name: "Selection",
          description:
            "Text selection: by character, word, and line, including selection expansion. Usually used together with Shift-style modifiers.",
        },
        sym: {
          name: "Symbols",
          description:
            "Symbols and punctuation that are inconvenient to type with the left hand on a regular layout (! {'@'} # $ % ^ & * {'|'} ~ `).",
        },
        fkeys: {
          name: "F keys",
          description:
            "Function-key layer for IDEs, debuggers, and system shortcuts.",
        },
        win: {
          name: "Windows",
          description:
            "Window manager and virtual desktops: desktop switching, moving windows, and tiling actions.",
        },
        space: {
          name: "Space",
          description:
            "Space-held layer for additional actions without moving hands away from the home row.",
        },
      },
    },
  },
  rules: {
    title: "Rules and triggers",
    subtitle:
      "Configure the logic: how physical keys activate layers or trigger actions.",
    addBtn: "Add rule",
    empty:
      'No rules yet. Click "Add rule" to define how a physical key should behave.',
    keyLabel: "Trigger key",
    keyHint: "Physical key that triggers this rule on press or hold.",
    keyPh: "pick a key",
    layerLabel: "Layer (hold)",
    layerHint:
      'Layer that activates while the key is held. It can be combined with "Hold action", for example to activate a layer and hold Alt at the same time.',
    clearLayer: "Clear layer",
    createLayer: "Create layer",
    tapLabel: "Tap action",
    tapHint:
      "Action performed on a short key press (released before the hold timeout elapses). Note: when a double-tap action is set, a single tap is delayed by the double-tap window to disambiguate it from the first press of a double tap.",
    tapPh: "no action",
    holdActionLabel: "Hold action",
    holdActionHint:
      'What happens while the key is held. "Native" = the physical key itself acts as held (e.g. ShiftLeft stays a Shift modifier). "None" = swallowed, nothing happens. "Action" lets you hold a different keystroke instead (e.g. MetaLeft can behave like a held ControlLeft). It can be combined with a Layer so the rule both activates the layer and holds the selected key.',
    holdActionPh: "pick a keystroke",
    modeNative: "Native",
    modeNativeDefault: "Native (default)",
    modeNone: "None (swallow)",
    modeAction: "Action",
    doubleTapLabel: "Double-tap action",
    doubleTapHint:
      "Action performed when the key is pressed twice in quick succession (second key-down within the double-tap window after a short first press). Fires on the second press; no need to release the key first.",
    doubleTapPh: "no action",
    holdLabel: "Hold ms",
    holdHint:
      "Individual hold timeout for this rule. Defaults to the value from Settings.",
    doubleTapWindowLabel: "Double-tap ms",
    doubleTapWindowHint:
      "Max time between first release and second key-down to recognise a double tap. Defaults to the value from Settings.",
    deleteRule: "Delete rule",
    keyRequired: "Pick a trigger key.",
    confirmDeleteTitle: "Delete rule?",
    confirmDeleteBody:
      "This rule already has a trigger key. Delete it permanently?",
    newLayerTitle: "New layer",
    layerName: "Layer name",
    layerNamePh: "For example: Navigation",
    layerDesc: "Description (optional)",
    layerDescPh: "Shortly: what this layer is for",
  },
  keymap: {
    layerLabel: "Layer",
    keyViewLabels: "US layout",
    keyViewCodes: "Key codes",
    keyViewNumeric: "Numeric codes",
    newLayer: "New layer",
    edit: "Edit",
    addDescription: "Add description",
    delete: "Delete",
    keyboardTitle: "Keyboard",
    leftHand: "Left hand",
    rightHand: "Right hand",
    extrasTitle: "Additional keys",
    extrasSub:
      "Mouse buttons, media keys or any other triggers not on the main keyboard.",
    addExtra: "Add",
    extrasEmpty: "No extra keys for this layer yet.",
    extraKeyLabel: "Key",
    extraKeyHint:
      "Trigger key: mouse button, media key or any other key from the full list.",
    extraActionLabel: "Action",
    extraActionHint: "What this key does while the layer is active.",
    moveExtraUp: "Move up",
    moveExtraDown: "Move down",
    deleteExtra: "Delete",
    deleteLayerTitle: "Delete layer “{name}”",
    deleteLayerBody:
      "This removes the layer, its keymap, and clears references to it from layer rules.",
    emptyTitle: "No layers yet",
    emptyBody:
      "Create an extra layer to define its keymap and activate it from hold rules.",
    editLayerTitle: "Edit layer",
    editLayerAria: "Edit layer {name}",
    editTitle: "Edit: {label}",
    keyCode: "Key code: {code}",
    editKeyAria: "Edit {label}",
  },
  macros: {
    title: "User macros",
    subtitle:
      "A sequence of steps. Each step — one key chord or system action. Steps run in order: the previous chord is fully released, the pause is honored, then the next is pressed.",
    addBtn: "New macro",
    addDisabled: "Fix the existing macro errors first",
    empty: 'No macros yet. Click "New macro" to create the first one.',
    defaultName: "New macro",
    nameLabel: "Name",
    nameHint: "Human-readable macro name, shown in picker lists.",
    namePh: "Macro name",
    idLabel: "ID",
    idHint:
      'Unique identifier for references like macro:<id>. To bind the macro to a key, pick this action on the "Layers" or "Rules" tab. Only change the ID if no key uses this macro.',
    idPh: "id",
    copyId: "Copy ID",
    idErrors: {
      empty: "ID cannot be empty.",
      format: 'Only Latin letters, digits, "_" and "-", up to 64 chars.',
      dupUser: "This ID is already used by another user macro.",
      dupSystem: 'This ID is already taken by system macro "{name}".',
    },
    stepErrors: {
      nestedMacro: "Nested macro references are not supported in macro steps.",
    },
    stepPauseLabel: "Pause between steps",
    stepPauseHint:
      "Per-macro pause between steps. Defaults to the Settings value.",
    modDelayLabel: "Modifier delay",
    modDelayHint:
      "Time between pressing a modifier and the main key within one step. Defaults to the Settings value.",
    usedIn: "Used in:",
    steps: "Steps",
    addStep: "Add step",
    stepsEmpty: "No steps yet. Each step — one chord or action.",
    stepPh: "Pick an action",
    moveUp: "Up",
    moveDown: "Down",
    deleteStep: "Delete step",
    deleteMacro: "Delete macro",
    assignHint:
      'To bind the macro to a key, pick the action {ref} on the "Layers" or "Rules" tab.',
    copySuffix: "(copy)",
    systemTitle: "System macros",
    systemSub:
      "Built-in macros that cannot be edited. To customize, create a user macro based on one.",
    systemEmpty: "No system macros.",
    colId: "ID",
    colName: "Name",
    colSteps: "Steps",
    cloneBtn: "Create based on",
    confirmDeleteTitle: "Delete macro?",
    confirmDeleteBody:
      "The macro will be deleted. References {ref} will stop working.",
  },
  commands: {
    title: "Commands",
    subtitle:
      "Reusable shell commands for the current layout. On Linux they run through `sh -lc`.",
    addBtn: "New command",
    addDisabled: "Fix the existing command errors first",
    empty: 'No commands yet. Click "New command" to create the first one.',
    defaultName: "New command",
    nameLabel: "Name",
    nameHint: "Human-readable name shown in picker lists.",
    namePh: "Command name",
    idLabel: "ID",
    idHint:
      'Unique identifier for references like cmd:<id>. To bind the command to a key, pick this action on the "Layers" or "Rules" tab.',
    idPh: "id",
    copyId: "Copy ID",
    linuxLabel: "Linux command",
    linuxHint: "Shell command to run on Linux. It is passed to `sh -lc` as-is.",
    linuxPh: "playerctl play-pause",
    usedIn: "Used in:",
    moveUp: "Up",
    moveDown: "Down",
    deleteCommand: "Delete command",
    idErrors: {
      empty: "ID cannot be empty.",
      format: 'Only Latin letters, digits, "_" and "-", up to 64 chars.',
      dupUser: "This ID is already used by another command.",
    },
    linuxErrors: {
      empty: "Linux command cannot be empty.",
    },
    confirmDeleteTitle: "Delete command?",
    confirmDeleteBody:
      "The command will be deleted. References {ref} will stop working.",
  },
  settings: {
    mapperTitle: "Key-mapper",
    keyboardLabel: "Keyboard",
    keyboardHelp:
      "Pick the physical device whose events should be intercepted.",
    devicePh: "Not selected",
    refreshDevices: "Refresh list",
    start: "Start",
    stop: "Stop",
    mapperHint:
      "The mapper reads events directly from {input} and emits via {uinput}. You need access to those devices — see README (group {group} and a udev rule for {uinputDev}).",
    generalTitle: "General",
    behaviorTitle: "Global behavior defaults",
    appearance: "Appearance",
    appearanceHint:
      '"Use system" mode follows the OS {pref} and switches automatically. Currently the {mode} theme is active.',
    appearanceLight: "light",
    appearanceDark: "dark",
    appearanceItems: {
      system: "Use system",
      light: "Light",
      dark: "Dark",
    },
    language: "Language",
    languageHint:
      'Interface language. "Auto" picks a language similar to the system one, falling back to English.',
    languageAuto: "Auto (system)",
    languageAutoResolved: "Auto (system → {resolved})",
    launchOnStartup: "Launch on startup",
    launchOnStartupHint:
      "Not yet implemented — the switch is saved in config but does not register autostart.",
    stubBadge: "Stub",
    platformTitle: "Platform diagnostics",
    platformHint:
      'Shows backend support separately from runtime availability, so the UI does not confuse "implemented" with "usable right now".',
    platformBackend: "Backend",
    platformBackendHint: "What the current OS/backend is designed to support.",
    platformRuntime: "Runtime",
    platformCapability: "Capability",
    platformSupportedColumn: "Supported",
    platformAvailableColumn: "Available now",
    platformLegend:
      "Supported = implemented for this backend. Available now = passed the current runtime probe in this environment.",
    platformUnavailable:
      "Platform diagnostics are available only in the desktop build.",
    supportedYes: "supported",
    supportedNo: "not supported",
    availableNow: "available now",
    unavailableNow: "unavailable now",
    platformCapabilities: {
      keyInterception: "Key interception",
      literalInjection: "Literal injection",
      layoutDetection: "Layout detection",
      systemActions: "System actions",
    },
    holdTimeout: "Default hold timeout, ms",
    holdTimeoutHint:
      "Decides single tap vs. layer hold. If the key is released before the timeout — the tap action fires; if held longer — the layer activates. Used by rules that do not define their own value.",
    doubleTapTimeout: "Default double-tap window, ms",
    doubleTapTimeoutHint:
      "Max time between first release and second key-down to recognise a double tap. When a rule has a double-tap action, its single tap is delayed by this value to disambiguate. Used by rules that do not define their own value.",
    stepPauseLabel: "Default macro step pause, ms",
    stepPauseHint:
      "Global default. Used when a macro step does not set its own.",
    modDelayLabel: "Modifier delay, ms",
    modDelayHint:
      "Global value: how long to wait between pressing a modifier (Shift/Ctrl/...) and the main key within one step.",
    layoutsTitle: "Layouts",
    loadFailed: 'Failed to load layout "{name}"',
    activeBadge: "active",
    builtinBadge: "built-in",
    unsavedBadge: "unsaved",
    applyBtn: "Apply",
    saveCurrent: "Save",
    saveAs: "Save As",
    deleteAria: "Delete",
    dirtyBadgeTitle: "The current layout has unsaved changes.",
    dirtyBadgeBody:
      "Switching to another layout will replace the rules, layers, keymap and macros. Save the current as a user layout to keep it.",
    resetHint:
      "Reset everything: clear layers, rules, keymap and macros. App settings are preserved.",
    resetBtn: "Reset all",
    emptyLayoutName: "Empty layout",
    userLayoutsDir: "Your layouts:",
    configTitle: "Settings files",
    settingsDirPath: "Settings directory",
    layoutsPath: "Your layouts",
    confirmApplyTitle: 'Switch to "{label}"?',
    confirmApplyBody:
      "The current layers, rules, keymap and macros will be replaced. App settings are preserved.",
    dirtyWarnTitle: "Warning: the current layout has unsaved changes.",
    dirtyWarnBody:
      "If you proceed, they will be lost permanently. Go back and click {btn} to write them to disk.",
    loseAndSwitch: "Lose changes and switch",
    switch: "Switch",
    saveModalTitle: "Save layout",
    nameLabel: "Layout name",
    namePh: "my-layout",
    saveHint:
      "The file will be saved as {path}. An existing layout with the same name will be overwritten.",
    saveErrorEmpty: "Enter a layout name.",
    deleteTitle: 'Delete "{name}"?',
    deleteBody:
      "The layout file will be deleted from disk. This cannot be undone. The currently active layout does not change.",
  },
  picker: {
    currentValue: "Current value",
    valuePh: "E.g.: Ctrl+C, Escape, macro:copyLine, cmd:toggleMusic",
    chooseAction: "Choose action",
    chooseKey: "Pick key",
    titleAction: "Pick action",
    titleKey: "Pick key",
    emptyCategory: "This category is empty.",
    clearAria: "Clear",
    physicalKeyHint:
      "Letters and symbols here mean the physical key code on a standard English layout, not the character from the user's current layout.",
    chordHint:
      "If you need a trigger chord, type it manually in the field above. Examples: Ctrl+KeyH, Shift+Space, AltLeft+KeyJ.",
  },
  categories: {
    special: "Special",
    lettersSymbols: "Letters and symbols",
    digitsNumpad: "Digits and numpad",
    media: "Media",
    mouse: "Mouse",
    other: "Other",
    commands: "Commands",
    macros: "Macros",
    systemMacros: "System macros",
    system: "System",
  },
  systemActions: {
    switchDesktop: "Switch to desktop {n}",
    switchLayout: "Switch to layout {n}",
    taskEntry: "Activate task manager entry {n}",
    walkThroughWindowsAlternative: "Walk through windows: alternative",
    walkThroughWindowsCurrentApp: "Walk through windows of current application",
    showClipboardHistory: "Show clipboard history",
    volumeDown: "Decrease volume",
    volumeUp: "Increase volume",
    muteAudio: "Toggle mute",
    brightnessDown: "Decrease screen brightness",
    brightnessUp: "Increase screen brightness",
    windowClose: "Close active window",
    windowToNextDesktop: "Move window to next desktop",
    windowKeepAbove: "Keep window above others",
    windowMaximizeVertical: "Maximize window vertically",
    windowMaximizeHorizontal: "Maximize window horizontally",
    screenshot: "Take area screenshot",
    screenOff: "Turn off screen",
  },
  mapper: {
    desktopOnly:
      "The mapper is only available in the desktop build (pnpm tauri:dev).",
  },
  language: {
    name: "English",
  },
};
