// English (en-US) locale messages.
export default {
  app: {
    title: 'Left Hand Control',
    badge: 'Linux key-mapper',
    loading: 'Loading…',
    saved: 'Saved',
    saveFailedTitle: 'Failed to save changes',
    loadFailedBody:
      'The app could not load its configuration. Check the error below, then try again.',
    notSavedBadge: '• unsaved',
    dirtyTooltip:
      'The current layout has unsaved changes. Save it to My Layouts before switching if you want to keep them.',
    customLayout: 'Custom layout',
    noLayout: 'No layout',
    switchToLight: 'Switch to light theme',
    switchToDark: 'Switch to dark theme',
    quit: 'Quit application',
    presetLabel: 'Preset:',
    layoutLanguageLabel: 'Language:',
    gameModeLabel: 'GAME',
    settingsTooltip: 'Settings',
    activeLayoutTooltip: 'Active layout',
    layoutsBack: 'Layouts',
    saveLayoutTooltip: 'Save "{name}"',
  },
  common: {
    notSet: 'Not set',
    cancel: 'Cancel',
    save: 'Save',
    delete: 'Delete',
    apply: 'Apply',
    create: 'Create',
    clear: 'Clear',
    edit: 'Edit',
    confirm: 'Confirm',
    refresh: 'Refresh',
    retry: 'Retry',
    add: 'Add',
    close: 'Close',
    copy: 'Copy',
    copyFailed: 'Copy failed',
    duplicate: 'Duplicate',
    copied: 'Copied',
    reset: 'Reset to default',
    override: 'Override',
    ms: 'ms',
    active: 'active',
    stopped: 'stopped',
    none: '— none —',
    asInSystem: 'Use system',
    backToTop: 'back to top',
    page: 'Page',
    moveUp: 'Move up',
    moveDown: 'Move down',
    drag: 'Drag',
    expand: 'Expand',
    collapse: 'Collapse',
    usingGlobalDefault: 'Global default: {value}. Click to override.',
  },
  tabs: {
    home: 'Home',
    layouts: 'My Layouts',
    rules: 'Rules',
    keymap: 'Layers',
    macros: 'Macros',
    commands: 'Commands',
    settings: 'Settings',
    quickActions: 'Quick Actions',
    emoji: 'Emoji & Symbols',
  },
  home: {
    title: 'Home',
    subtitle:
      'Choose the active layout and check what is supported on this system.',
    infoTitle: 'Quick start',
    infoSubtitle: 'Everything you need to know to get started.',
    layoutsHintTitle: 'Using the library',
    layoutsHintBody:
      'Create layouts, switch between them, and save edits as separate layout files.',
    modeLabel: 'Layout mode',
    modeManual: 'Manual',
    modeAuto: 'Auto',
    modeManualHint:
      'You choose the active layout. Its allow/block conditions still decide whether input is remapped.',
    modeAutoHint:
      'The app picks the first matching enabled layout from the list below.',
    modeInfoAria: 'How layout mode works',
    modeManualInfo:
      'In manual mode, you choose the active layout yourself. It stays active until you choose another one. Its allow/block conditions still apply: when they block the layout, input passes through unchanged.',
    modeAutoInfo:
      'In auto mode, only enabled layouts participate. Order matters: the app checks the list from top to bottom and activates the first layout allowed by its conditions. Allow conditions limit where a layout can activate. Block conditions take priority. With only block conditions, the layout works everywhere except the blocked cases. A layout with no conditions matches everywhere. If nothing matches, no layout is active and input passes through unchanged.',
    activeLayoutLabel: 'Active layout',
    activeLayoutNative: 'Native (no layout active)',
    manualNoActiveTitle: 'No active layout',
    manualNoActiveBody: 'Pick a layout manually so the mapper uses its rules.',
    whatIsItTitle: 'What does this app do?',
    whatIsItBody:
      'Left Hand Control intercepts your physical keyboard at the OS level and remaps keys in real time. You can turn rarely-used keys into powerful modifiers: hold CapsLock and the home row becomes arrow keys, navigation, or symbols.',
    howToStartTitle: 'How do I turn it on?',
    howToStartBody:
      "Open Settings → Key-mapper, select your keyboard, and press Start. The app needs access to /dev/input/event* (usually via the 'input' group) and /dev/uinput. See the README for the udev rule.",
    layersExplainedTitle: 'What are layers?',
    layersExplainedBody:
      "A layer is an alternate keymap that activates while you hold a specific key. For example, in the built-in layout holding CapsLock turns J/K/L/I into ArrowLeft/ArrowDown/ArrowRight/ArrowUp, U/O into Home/End, and so on. Release the key and you're back to normal typing.",
    rulesExplainedTitle: 'What are rules?',
    rulesExplainedBody:
      'Rules define what a physical key does. A rule can handle a short press, a hold, and a quick double press. You can also limit it by active application, game mode, or keyboard layout language.',
    builtInLayoutTitle: 'Try the built-in layout',
    builtInLayoutBody:
      "The app includes Ivan K's left-hand layout. It adds a navigation layer on CapsLock, a symbols layer on Right Alt, and a window-management layer on Left Alt. Start the mapper and try it in your editor.",
    fullDocsLabel: 'Read the full documentation →',
    howItWorksTitle: 'How it works on your system',
    platformDetected: 'Detected: {platform}',
    keyInterceptionLabel: 'Key remapping',
    keyInterceptionLinux:
      "Reads keyboard events from /dev/input/event* and re-emits modified events through /dev/uinput. Works on any Linux desktop and both X11 and Wayland. You need access to those devices — usually via the 'input' group and a udev rule for /dev/uinput.",
    keyInterceptionStub: 'Not yet implemented on this operating system.',
    textInjectionLabel: 'Text input',
    textInjectionLinux:
      'Literal text is injected through the xdg-desktop-portal RemoteDesktop backend. Requires the portal service running and a matching backend package (xdg-desktop-portal-kde on KDE, -gnome on GNOME, -wlr on Sway/wlroots).',
    textInjectionStub: 'Not yet implemented on this operating system.',
    systemActionsLabel: 'System actions',
    systemActionsLinuxKde:
      'Window management, virtual desktops, and other system functions use qdbus to call KDE Plasma D-Bus interfaces such as KWin. This is normally available on Plasma.',
    systemActionsLinuxOther:
      'System actions are not fully integrated with {desktop} yet. Some actions may be unavailable.',
    systemActionsStub: 'Not yet implemented on this operating system.',
    layoutDetectionLabel: 'Layout detection',
    layoutDetectionLinuxKde:
      'Keyboard layout changes are monitored via the org.kde.keyboard D-Bus service.',
    layoutDetectionLinuxOther:
      'Automatic layout detection is not yet integrated with {desktop}.',
    layoutDetectionStub: 'Not yet implemented on this operating system.',
    statusSupported: 'Supported',
    statusAvailable: 'Available',
    statusUnavailable: 'Unavailable',
  },
  welcome: {
    intro:
      'Welcome! Pick a starting layout — you can change it anytime in Settings.',
    recommended: 'recommended',
    useThis: 'Use this layout',
    emptyTitle: 'Empty layout',
    emptyDesc:
      'Start from scratch: no layers, rules, or macros. Use this if you want to build your own layout.',
    emptyBtn: 'Start empty',
    footnote: 'You can always change the chosen layout under {path}.',
    footnotePath: 'Settings → Layouts',
    loadError: 'Failed to load the built-in layout.',
    defaultEmptyFileName: 'My new layout',
    defaultIvanKFileName: 'Ivan K layout',
  },
  builtinLayouts: {
    ivank: {
      name: "Ivan K's left hand control",
      description:
        'Ready-made layout: CapsLock for navigation, Right Alt for symbols, Left Alt for window management. Includes IDE-oriented macros.',
      layers: {
        nav: {
          name: 'Navigation',
          description:
            'Navigation through text and code: arrows, Home/End, PageUp/Down, and movement by words and lines. Activated while CapsLock is held.',
        },
        sel: {
          name: 'Selection',
          description:
            'Text selection: by character, word, and line, including selection expansion. Usually used together with Shift-style modifiers.',
        },
        sym: {
          name: 'Symbols',
          description:
            "Symbols and punctuation that are inconvenient to type with the left hand on a regular layout (! {'@'} # $ % ^ & * {'|'} ~ `).",
        },
        fkeys: {
          name: 'F keys',
          description:
            'Function-key layer for IDEs, debuggers, and system shortcuts.',
        },
        win: {
          name: 'Windows',
          description:
            'Window manager and virtual desktops: desktop switching, moving windows, and tiling actions.',
        },
        space: {
          name: 'Space',
          description:
            'Space-held layer for additional actions without moving hands away from the home row.',
        },
      },
    },
  },
  rules: {
    title: 'Rules and triggers',
    subtitle: 'Define how physical keys activate layers or run actions.',
    addBtn: 'Add rule',
    empty:
      'No rules yet. Click "Add rule" to define how a physical key should behave.',
    keyLabel: 'Trigger key',
    keyHint: 'Physical key that triggers this rule on press or hold.',
    keyPh: 'Pick a key',
    layerLabel: 'Layer (hold)',
    layerHint:
      'Layer that stays active while the trigger key is held. You can combine it with a hold action, for example to activate a layer while holding Alt.',
    clearLayer: 'Clear layer',
    createLayer: 'Create layer',
    layerPh: 'Pick layer',
    tapLabel: 'Tap action',
    tapHint:
      'Runs when the trigger key is released before the hold timeout. If a double-tap action is set, the single tap waits for the double-tap window before firing.',
    tapPh: 'No action',
    holdActionLabel: 'Hold button',
    holdActionHint:
      'What happens while the trigger key is held. Use the physical key as-is, suppress it, or hold another key or shortcut instead. This can run together with a layer.',
    holdActionPh: 'Pick a keystroke',
    modeNative: 'Native',
    modeNativeDefault: 'Native (default)',
    modeNone: 'None (swallow)',
    modeAction: 'Action',
    holdModeNativeDefault: 'Use physical key',
    holdModeNone: 'Do nothing',
    holdModeAction: 'Hold another key',
    isolateLabel: 'Hold exclusions',
    isolateHint:
      'Keys in the active layer that temporarily release the held key. Separate multiple keys with commas.',
    isolatePh: 'KeyW, KeyC, Slash',
    doubleTapLabel: 'Double-tap action',
    doubleTapHint:
      'Runs when the trigger key is pressed twice quickly. It fires on the second key-down, before the second release.',
    doubleTapPh: 'No action',
    holdLabel: 'Hold timeout, ms',
    holdHint:
      'Individual hold timeout for this rule. Defaults to the value from Settings.',
    doubleTapWindowLabel: 'Double-tap ms',
    doubleTapWindowHint:
      'Max time between first release and second key-down to recognise a double tap. Defaults to the value from Settings.',
    deleteRule: 'Delete rule',
    conditionsLabel: 'Conditions',
    conditionsHint:
      'Limit this rule by game mode, keyboard layout, or active application.',
    manualActiveLabel: 'Active Layout (Manual)',
    manualActiveHint: 'Click activate to use this layout right now.',
    activateBtn: 'Activate',
    layoutConditionsTitle: 'Layout Conditions',
    layoutConditionsSubtitle:
      'Auto-mode conditions for the current layout ({name})',
    saveToConfigureAuto: 'Save the layout to configure auto-mode conditions.',
    autoIncludeLabel: 'Include in auto mode',
    autoIncludeHint:
      'Enabled layouts can be picked automatically. With no conditions, the layout matches everywhere.',
    whitelistPrefix: 'Active when',
    blacklistPrefix: 'Inactive when',
    conditionsNone: '— any —',
    conditionsBtn: 'Conditions…',
    editConditions: 'Edit conditions',
    clearConditions: 'Clear conditions',
    gameModeLabel: 'Game Mode',
    gameModeHint: 'Run this rule only when game mode is in the selected state.',
    gameModeIgnore: 'Ignore',
    gameModeOn: 'On',
    gameModeOff: 'Off',
    gameModeOnSummary: 'game mode on',
    gameModeOffSummary: 'game mode off',
    layoutsLabel: 'Language Layouts',
    layoutsHint:
      'Multiple layouts can be selected. The rule will run when any of the selected layouts is active.',
    anyLayout: 'Any layout',
    noLayoutsDetected:
      'No keyboard layouts detected. Layout detection is available only in the desktop app.',
    appsLabel: 'Active application',
    appsHint:
      'Text fragments (case-insensitive) searched in the focused window title or app id. Empty list means "do not check".',
    appsPlaceholder: 'e.g. firefox, Steam, .exe',
    appsWhitelistLabel: 'Triggers for apps',
    appsWhitelistHint:
      'Rule fires only when the focused window title or app id contains at least one listed text fragment. Leave empty to ignore.',
    appsBlacklistLabel: 'Skipped for apps',
    appsBlacklistHint:
      'Rule is blocked when the focused window title or app id contains at least one listed text fragment. Takes precedence over allowed applications.',
    appsWhitelistCount: '{count} included',
    appsBlacklistCount: '{count} excluded',
    enableRule: 'Enable rule',
    disableRule: 'Disable rule',
    keyRequired: 'Pick a trigger key.',
    draftsTitle: 'Draft rules will be ignored',
    draftsDescription:
      '{count} enabled rule has no trigger key. It is saved, but will not run.',
    issuesTitle: 'Fix rule conflicts before starting',
    issueMessages: {
      missingTrigger: 'Saved draft: add a trigger key to make this rule run.',
      invalidTrigger: 'Trigger {trigger} cannot be used for a rule.',
      duplicateTrigger:
        'Trigger {trigger} is used by more than one active rule.',
      unknownLayer: 'Rule {trigger} points to a layer that no longer exists.',
      invalidTapAction: 'Rule {trigger} has an invalid tap action.',
      invalidHoldAction:
        'Rule {trigger} has an invalid hold action. Hold actions must be a key or chord.',
      invalidDoubleTapAction:
        'Rule {trigger} has an invalid double-tap action.',
    },
    confirmDeleteTitle: 'Delete rule?',
    confirmDeleteBody:
      'This rule already has a trigger key. Delete it permanently?',
    newLayerTitle: 'New layer',
    layerName: 'Layer name',
    layerNamePh: 'For example: Navigation',
    layerDesc: 'Description (optional)',
    layerDescPh: 'Short description of this layer',
    layerCreated: 'Layer "{name}" created',
  },
  keymap: {
    layerLabel: 'Layer',
    keyViewLabels: 'US layout',
    keyViewCodes: 'Internal codes',
    keyViewNumeric: 'System codes',
    newLayer: 'New layer',
    edit: 'Edit',
    addDescription: 'Add description',
    delete: 'Delete',
    keyboardTitle: 'Keyboard',
    leftHand: 'Left hand',
    rightHand: 'Right hand',
    extrasTitle: 'Additional keys',
    extrasSub:
      'Mouse buttons, media keys, and other triggers outside the main keyboard grid.',
    addExtra: 'Add',
    extrasEmpty: 'No extra keys for this layer yet.',
    extraKeyLabel: 'Key',
    extraKeyHint:
      'Trigger key: mouse button, media key or any other key from the full list.',
    extraActionLabel: 'Action',
    extraActionHint: 'Action sent by this key while the layer is active.',
    moveExtraUp: 'Move up',
    moveExtraDown: 'Move down',
    deleteExtra: 'Delete',
    deleteLayerTitle: 'Delete layer “{name}”',
    deleteLayerBody:
      'This removes the layer, deletes its key bindings, and clears rules that activate it.',
    deleteLayerRulesHint_one:
      'One rule references this layer. It will be cleared.',
    deleteLayerRulesHint_other:
      '{count} rules reference this layer. They will be cleared.',
    emptyTitle: 'No layers yet',
    emptyBody:
      'Create a layer, assign keys to it, then activate it from a hold rule.',
    editLayerTitle: 'Edit layer',
    renameLayerTitle: 'Rename layer',
    renameLayerAria: 'Rename layer {name}',
    cloneLayerTitle: 'Duplicate layer',
    cloneLayerAria: 'Duplicate layer {name}',
    editLayerAria: 'Edit layer {name}',
    renameLayerTooltip: 'Rename layer',
    cloneLayerTooltip: 'Duplicate layer',
    deleteLayerTooltip: 'Delete layer',
    newLayerTooltip: 'Create new layer',
    editTitle: 'Edit: {label}',
    keyCode: 'Internal code: {code}',
    editKeyAria: 'Edit {label}',
    swallowAction: 'Ignore in this layer',
    swallowLabel: 'Ignore',
    clearLayerTitle: 'Clear layer',
    clearLayerBody: 'All key bindings in this layer will be removed.',
    layerCleared: 'Layer cleared',
    undoClear: 'Undo',
    clearExtrasTitle: 'Clear extra keys',
    clearExtrasBody: 'All extra keys will be removed.',
    extrasCleared: 'Extra keys cleared',
    clearKeyboardTooltip:
      'Clears key bindings only. Extra keys are not affected.',
    keyboardInfo:
      'Display modes affect labels only — the key code stays the same. "US layout" shows standard letter labels for visual reference. "Internal codes" shows app-specific codes (e.g. KeyA, ArrowLeft). "System codes" shows raw system event codes.',
  },
  macros: {
    title: 'User macros',
    subtitle:
      'Build reusable sequences from key chords, text actions, commands, or system actions. Steps run in order with the configured pause between them.',
    addBtn: 'New macro',
    addDisabled: 'Fix the existing macro errors first',
    empty: 'No macros yet. Click "New macro" to create the first one.',
    defaultName: 'New macro',
    nameLabel: 'Name',
    nameHint: 'Human-readable macro name, shown in picker lists.',
    namePh: 'Macro name',
    idLabel: 'ID',
    idHint:
      'Unique identifier used in references like macro:<id>. To bind this macro to a key, choose that action on the Layers or Rules tab. Change the ID only when nothing uses this macro.',
    idPh: 'id',
    copyId: 'Copy ID',
    idErrors: {
      empty: 'ID cannot be empty.',
      format: 'Only Latin letters, digits, "_" and "-", up to 64 chars.',
      dupUser: 'This ID is already used by another user macro.',
      dupSystem: 'This ID is already taken by system macro "{name}".',
    },
    stepErrors: {
      macroCycle: 'Macro references cannot form a cycle.',
      pauseFormat: 'Pause step must use pause:<milliseconds>.',
      pauseRange: 'Pause must be between 0 and 10000 ms.',
    },
    stepWarnings: {
      empty: 'Empty step — pick an action or remove it.',
    },
    stepPauseLabel: 'Pause between steps',
    stepPauseHint:
      'Per-macro pause between steps. Defaults to the Settings value.',
    modDelayLabel: 'Modifier delay',
    modDelayHint:
      'Time between pressing a modifier and the main key within one step. Defaults to the Settings value.',
    usedIn: 'Used in:',
    steps: 'Steps',
    addStep: 'Add step',
    addPauseStep: 'Add pause',
    pauseStep: 'Turn into pause',
    actionStep: 'Turn into action',
    stepsEmpty: 'No steps yet. Each step — one chord or action.',
    stepPh: 'Pick an action',
    moveUp: 'Up',
    moveDown: 'Down',
    deleteStep: 'Delete step',
    deleteMacro: 'Delete macro',
    assignHint:
      'To bind this macro to a key, choose {ref} on the Layers or Rules tab.',
    copySuffix: '(copy)',
    systemTitle: 'System macros',
    systemSub:
      'Built-in macros that cannot be edited. To customize, create a user macro based on one.',
    systemEmpty: 'No system macros.',
    colId: 'ID',
    colName: 'Name',
    colSteps: 'Steps',
    cloneBtn: 'Create based on',
    cloneSystemTooltip: 'Create user macro from this system macro',
    confirmDeleteTitle: 'Delete macro?',
    confirmDeleteBody:
      'The macro will be deleted. References {ref} will stop working.',
    confirmDeleteStepTitle: 'Delete step?',
    confirmDeleteStepBody: 'This step will be removed from the macro.',
  },
  quickActions: {
    title: 'Quick Actions',
    subtitle: 'Opened by the app:showQuickMenu1...5 actions.',
    empty: 'No quick actions yet. Add one to see it in the Quick Menu.',
    addBtn: 'Add Action',
    addPage: 'Add page',
    deletePage: 'Delete page',
    deletePageTitle: 'Page deletion',
    pageLabel: 'Page name',
    defaultName: 'New action',
    nameLabel: 'Label',
    namePh: 'e.g. Next Track',
    actionLabel: 'Action',
    actionPh: 'Choose an action',
    deleteAction: 'Delete action',
    clearAction: 'Clear',
    moveUp: 'Move up',
    moveDown: 'Move down',
    menuTabHint: 'Tab switches pages.',
    menuKeysHint: 'Q W E R T / A S D F G / Z X C V B run the selected action.',
    pageName: 'Page {n}',
    emptyCell: 'Empty',
    cellLabel: 'Key {key}',

    confirmDeletePageTitle: 'Delete page?',
    confirmDeletePageBody:
      'The page and all assigned cells on it will be deleted.',
  },
  emoji: {
    title: 'Emoji & Symbols',
    subtitle:
      'Opened by the app:showEmojiMenu1...5 actions. Each cell can hold an emoji, a UTF symbol (e.g. ≠), any other character (e.g. №), or a short phrase (e.g. "// TODO: ").',
    addPage: 'Add page',
    deletePage: 'Delete page',
    deletePageTitle: 'Page deletion',
    pageLabel: 'Page name',
    pageName: 'Page {n}',
    cellLabel: 'Cell {key}',
    customPlaceholder: 'Emoji, symbol, or any text',
    clearCell: 'Clear cell',
    cellTooLongTitle: 'Text is too long',
    cellTooLongBody: 'Maximum {max} characters per cell.',
    confirmDeletePageTitle: 'Delete page?',
    confirmDeletePageBody:
      'The page and all assigned cells on it will be deleted.',
    categories: {
      smileys: 'Smileys & Emotion',
      people: 'People & Gestures',
      symbols: 'Emoji Symbols',
      work: 'Work & Objects',
      nature: 'Animals & Nature',
      food: 'Food & Drink',
      travel: 'Travel & Places',
      utf: 'UTF Symbols',
    },
  },
  emojiMenu: {
    tabHint: 'Tab switches pages.',
    keysHint: 'Q W E R T / A S D F G / Z X C V B insert the selected cell.',
    insertFailedTitle: 'Failed to insert',
    insertFailedBody:
      'The text could not be inserted. Start the mapper and check that xdg-desktop-portal is available.',
  },
  commands: {
    title: 'Commands',
    subtitle:
      'Reusable shell commands for the current layout. On Linux they run through sh -lc after you explicitly allow them.',
    subtitleLinux:
      'Reusable shell commands for the current layout. On Linux they run through sh -lc after you explicitly allow them.',
    subtitleWindows:
      'Commands are saved in the layout, but shell command execution on Windows is not implemented yet.',
    subtitleMacos:
      'Commands are saved in the layout, but shell command execution on macOS is not implemented yet.',
    subtitleUnknown:
      'Reusable commands for the current layout. Execution support depends on the current platform.',
    addBtn: 'New command',
    addDisabled: 'Fix the existing command errors first',
    empty: 'No commands yet. Click "New command" to create the first one.',
    defaultName: 'New command',
    nameLabel: 'Name',
    nameHint: 'Human-readable name shown in picker lists.',
    namePh: 'Command name',
    idLabel: 'ID',
    idHint:
      'Unique identifier used in references like cmd:<id>. To bind this command to a key, choose that action on the Layers or Rules tab.',
    idPh: 'id',
    copyId: 'Copy ID',
    linuxLabel: 'Linux shell script',
    linuxHint:
      'Passed to sh -lc as-is. Multiline scripts, variables, pipes, conditions, and loops are supported. Read scripts from shared layouts carefully before allowing them.',
    linuxPh: "playerctl play-pause\nnotify-send 'Playback toggled'",
    approvalTitle: 'Shell commands require approval',
    approvalBody:
      'This layout contains shell scripts. Read every script before allowing them, especially in layouts from other people. Approval applies to the reviewed command definitions and resets when the command set, IDs, or Linux scripts change.',
    approveBtn: 'Allow commands',
    approved:
      'Shell commands are allowed for this version of the current layout.',
    revokeBtn: 'Disable',
    approvalToast:
      'Shell commands are blocked until you review and allow them on the Commands tab.',
    usedIn: 'Used in:',
    moveUp: 'Up',
    moveDown: 'Down',
    deleteCommand: 'Delete command',
    idErrors: {
      empty: 'ID cannot be empty.',
      format: 'Only Latin letters, digits, "_" and "-", up to 64 chars.',
      dupUser: 'This ID is already used by another command.',
    },
    linuxErrors: {
      empty: 'Linux command cannot be empty.',
    },
    confirmDeleteTitle: 'Delete command?',
    confirmDeleteBody:
      'The command will be deleted. Keys or macros that reference {ref} will stop working.',
  },
  settings: {
    mapperTitle: 'Key-mapper',
    mapperInfo: 'Key-mapper information',
    keyboardLabel: 'Keyboard',
    keyboardHelp:
      'Pick the physical device whose events should be intercepted.',
    devicePh: 'Not selected',
    mouseLabel: 'Mouse (optional)',
    mouseHelp:
      'Select the mouse so the mapper can see extra buttons and clicks. Pointer movement is not affected.',
    mouseNativeActionNote:
      'The mouse is not fully intercepted yet: a remapped extra button can emit the new action, but its native system or app action may still run. If that gets in the way, disable the native action for that button in your system, browser, or driver settings.',
    mouseDevicePh: 'Not selected (no mouse support)',
    refreshDevices: 'Refresh list',
    refreshDevicesTooltip: 'Refresh device list',
    keyboardLikeDevices: 'Looks like a keyboard',
    mouseLikeDevices: 'Looks like a mouse',
    otherDevices: 'Other devices',
    manualDeviceGroup: 'Manual',
    manualDeviceOption: 'Enter device path manually',
    manualDevicePlaceholder: '/dev/input/eventX',
    copyPathTooltip: 'Copy path',
    start: 'Start',
    startDisabledTooltip: 'Select a keyboard in Settings to start the mapper.',
    stop: 'Stop',
    mapperStartFailed: 'Could not start mapper',
    mapperHint:
      'The mapper reads from {input} and emits through {uinput}. If Start fails, grant access to those devices: usually the {group} group plus a udev rule for {uinputDev}.',
    generalTitle: 'General',
    behaviorTitle: 'Behavior defaults',
    appearance: 'Theme',
    appearanceItems: {
      system: 'Use system',
      light: 'Light',
      dark: 'Dark',
    },
    language: 'Language',
    languageAuto: 'Auto (system)',
    languageAutoResolved: 'Auto (system → {resolved})',
    launchOnStartup: 'Launch on startup',
    launchOnStartupHint:
      'Saved in settings, but autostart registration is not implemented yet.',
    stubBadge: 'Stub',
    issues: {
      bannerErrorTitle: 'Some features need attention',
      bannerWarningTitle: 'Some features are limited on this system',
      platformCheckTitle: 'Could not check platform requirements',
      mapperStartTitle: 'The mapper cannot start right now',
      mapperStartBody:
        'Grant access to /dev/input/event* and /dev/uinput, then try starting the mapper again.',
      literalInjectionTitle: 'Literal text output may be unavailable',
      literalInjectionBody:
        'Text actions depend on xdg-desktop-portal RemoteDesktop and may require a system permission prompt.',
      layoutDetectionUnsupportedTitle:
        'Automatic layout detection is not implemented here yet',
      layoutDetectionUnsupportedBody:
        'Automatic keyboard-layout detection is not implemented for {desktop} yet.',
      layoutDetectionUnavailableTitle:
        'Automatic layout detection is unavailable right now',
      layoutDetectionUnavailableBody:
        'Layout detection should work on {desktop}, but the runtime check failed.',
      systemActionsUnsupportedTitle:
        'Some system actions are not implemented here yet',
      systemActionsUnsupportedBody:
        'System actions are not implemented for {desktop} yet.',
      systemActionsUnavailableTitle:
        'Some system actions are unavailable right now',
      systemActionsUnavailableBody:
        'System actions should work on {desktop}, but the runtime check failed.',
    },
    holdTimeout: 'Default hold timeout, ms',
    holdTimeoutHint:
      'Used by rules without their own value. Release before this timeout to run the tap action; hold longer to activate the hold behavior.',
    doubleTapTimeout: 'Default double-tap window, ms',
    doubleTapTimeoutHint:
      'Used by rules without their own value. A double tap is recognized when the second key-down happens within this time after the first release. Single taps wait for this window when a double-tap action exists.',
    stepPauseLabel: 'Default macro step pause, ms',
    stepPauseHint:
      'Global default. Used when a macro step does not set its own.',
    modDelayLabel: 'Modifier delay, ms',
    modDelayHint:
      'Global value: how long to wait between pressing a modifier (Shift/Ctrl/...) and the main key within one step.',
    layoutsTitle: 'Layouts',
    emptyLayoutsTitle: 'No saved layouts yet',
    emptyLayoutsBody: 'Create your first layout using the buttons above.',
    loadFailed: 'Failed to load layout "{name}"',
    layoutApplied: 'Layout "{name}" applied',
    activeBadge: 'active',
    unsavedBadge: 'unsaved',
    editingBadge: 'Editing',
    loadBtn: 'Load',
    openLayoutBtn: 'Open',
    saveCurrent: 'Save',
    saveAs: 'Save as',
    newLayoutBtn: 'New',
    newEmptyLayoutBtn: 'New empty',
    newFromIvanKBtn: 'New from Ivan K',
    deleteAria: 'Delete',
    editLayoutAria: 'Edit layout {name}',
    renameLayoutAria: 'Rename layout {name}',
    dirtyBadgeTitle: 'The current layout has unsaved changes.',
    whitelist: 'Allowed conditions',
    whitelistTitle: 'Active when ...',
    whitelistHint:
      'The layout will only activate when all of these conditions are met.',
    blacklist: 'Blocked conditions',
    blacklistTitle: 'Inactive when ...',
    blacklistHint:
      'The layout will never activate when any of these conditions are met. If only blocked conditions are set, the layout is allowed everywhere else.',
    defaultBadge: 'default',
    inAutoBadge: 'in auto',
    moveLayoutUpAria: 'Move {name} up',
    moveLayoutDownAria: 'Move {name} down',
    dirtyBadgeBody:
      'Switching layouts replaces rules, layers, key bindings, macros, commands, quick actions, and emoji pages. Save the current layout first if you want to keep these changes.',
    resetUnsavedBtn: 'Reset changes',
    resetUnsavedTitle: 'Reset unsaved changes?',
    resetUnsavedBody:
      'Unsaved edits will be discarded and the current layout will be restored from disk.',
    resetHint:
      'Reset the current layout data: layers, rules, key bindings, macros, commands, quick actions, and emoji pages. App settings are preserved.',
    resetBtn: 'Reset all',
    emptyLayoutName: 'Empty layout',
    userLayoutsDir: 'Your layouts:',
    configTitle: 'Settings files',
    settingsDirPath: 'Settings directory',
    layoutsPath: 'Your layouts',
    confirmApplyTitle: 'Switch to "{label}"?',
    confirmApplyBody:
      'Current layout data will be replaced: layers, rules, key bindings, macros, commands, quick actions, and emoji pages. App settings are preserved.',
    dirtyWarnTitle: 'Warning: the current layout has unsaved changes.',
    dirtyWarnBody:
      'If you continue, they will be lost. Go back and click {btn} to write them to disk.',
    loseAndSwitch: 'Lose changes and switch',
    switch: 'Switch',
    saveModalTitle: 'Save layout',
    editLayoutTitle: 'Edit layout',
    renameLayoutTitle: 'Rename layout',
    editDescriptionTitle: 'Edit description',
    nameLabel: 'Layout name',
    namePh: 'my-layout',
    descriptionLabel: 'Description',
    descriptionPh: 'Short description of this layout',
    saveHint:
      'The file will be saved as {path}. An existing layout with the same name will be overwritten.',
    saveErrorEmpty: 'Enter a layout name.',
    saveErrorInvalidName:
      'The file name contains unsupported characters. Do not use \\ / : * ? " < > | and do not start the name with a dot.',
    overwriteTitle: 'Overwrite "{name}"?',
    overwriteBody:
      'A layout file with this name already exists. Replacing it cannot be undone.',
    deleteTitle: 'Delete "{name}"?',
    deleteBody:
      'The layout file will be deleted from disk. This cannot be undone. The active in-memory layout will not change.',
    outputPrincipleTitle: 'How output works',
    outputPrincipleBody1:
      'The mapper does not try to reinterpret already assembled shortcuts. It sends the system the final result: a single key or a shortcut such as Ctrl+KeyZ, Ctrl+KeyC, or Shift+Tab.',
    outputPrincipleBody2:
      'Because of that, the target app usually receives exactly the shortcut you configured in the layout. If an action is set to Ctrl+KeyZ, the system will see Ctrl+KeyZ even if the physical Z key is remapped to something else inside the layout.',
    gameModeTitle: 'Game Mode',
    gameModeSubtitle:
      'Used for auto-switch layout conditions and trigger rules.',
    gameModeUseGamemoded: 'Use gamemoded',
    gameModeUseGamemodedHint:
      'Not supported by all games — only works when the game runs through gamemoded (usually Steam, Lutris).',
    gameModeUseFullscreen: 'Detect fullscreen windows',
    gameModeUseFullscreenHint:
      'Triggers for any fullscreen application, including media players.',
    gameModeActive: 'Game Mode active',
    gameModeInactive: 'Game Mode',
    gameModeInfo: 'Game mode detection is enabled.',
    gameModeDisabledInfo:
      'Game mode detection is disabled. Conditions that check game mode will behave as ignored.',
    gameModeTitleHint:
      'Game mode is detected via running processes, fullscreen windows, or gamemoded. This state can be used in layout and rule conditions.',
    gameModeAdvanced: 'Advanced',
    gameModeProcessPlaceholder: 'Example: steam_app, cs2, eldenring.exe',
    gameModeAddProcess: 'Add',
    gameModeRemoveProcess: 'Remove process',
    gameModeOnlyActiveWindow: 'Only active window',
    gameModeOnlyActiveWindowHint:
      'Check only the active window app and title. When off, the rule searches all running processes.',
    gameModeListsTitle: 'Application blacklist and whitelist',
    gameModeListsHint:
      'The whitelist enables game mode when at least one app has a process name, app id, or window title containing a listed text fragment. The blacklist disables game mode using the same match and takes priority over the whitelist, gamemoded, and fullscreen detection. Items inside each list use logical OR.',
    gameModeProcessNameHint:
      'Enter part of a process name, app id, or window title. The full name is not required. An empty required field is shown as invalid and ignored by game mode detection.',
    gameModeWhitelistTitle: 'Active when running ...',
    gameModeBlacklistTitle: 'Inactive when running ...',
    system: {
      title: 'System / Troubleshooting',
      waylandNote:
        'Use these options when text actions behave differently on your desktop.',
      textModeLabel: 'Text injection method (Linux/Wayland)',
      textModeHint:
        'Controls how text: actions are sent. If characters come out wrong, for example with a non-Latin layout active, try the clipboard method.',
      textModeLibei: 'libei + clipboard fallback (default)',
      textModeLibeiHint:
        'Uses libei (EI protocol) for keycode injection. If any character is not in the current keyboard layout, the entire text is sent via clipboard (wl-copy + Ctrl+V).',
      textModeLibeiPure: 'libei (pure)',
      textModeLibeiPureHint:
        'Uses libei for keycode injection only. Characters not present in the active layout are skipped silently. Use this if you want libei without touching the clipboard.',
      textModeKeycode: 'XKB keycode',
      textModeKeycodeHint:
        'Reads the current keyboard layout and injects characters as keycodes with the correct modifiers. Does not touch the clipboard.',
      textModeClipboard: 'Clipboard (via wl-copy)',
      textModeClipboardHint:
        'Copies text via wl-copy and pastes with Ctrl+V through the RemoteDesktop portal. Known limitation: Ctrl+V may not fire on KDE with a non-Latin layout active (use libei + clipboard fallback instead).',
      textModeYdotool: 'ydotool-compatible',
      textModeYdotoolHint:
        'Runs a ydotool-compatible executable as `type <text>`. Requires ydotoold and access to /dev/uinput.',
      ydotoolPathLabel: 'ydotool executable',
      ydotoolPathHint:
        'Leave empty to use ydotool from PATH, or set an absolute path/custom wrapper.',
      ydotoolPathPlaceholder: 'ydotool or /usr/bin/ydotool',
      textModeXdotool: 'xdotool (X11/XWayland)',
      textModeXdotoolHint:
        'Runs xdotool as `type --clearmodifiers <text>`. Works on X11 and XWayland sessions. No daemon required.',
      xdotoolPathLabel: 'xdotool executable',
      xdotoolPathHint:
        'Leave empty to use xdotool from PATH, or set an absolute path.',
      xdotoolPathPlaceholder: 'xdotool or /usr/bin/xdotool',
    },
  },
  picker: {
    currentValue: 'Current value',
    currentValueWithValue: 'Previously selected: {value}',
    currentValueEmpty: 'No value was selected before opening',
    valuePh:
      'E.g.: Ctrl+KeyC, Escape, KeyA, macro:copyLine, cmd:toggleMusic, text:TODO: ',
    textPh: 'Type the text to insert. Example: TODO: ',
    chooseAction: 'Choose action',
    chooseKey: 'Pick key',
    titleAction: 'Pick action',
    titleKey: 'Pick key',
    emptyCategory: 'This category is empty.',
    invalidValue:
      'Use canonical action syntax only: KeyA, Digit1, ArrowLeft, Ctrl+KeyC, macro:<id>, cmd:<id>, sys:<id>, app:<id>, or text:<text>.',
    clearAria: 'Clear',
    textHint:
      'Text actions are stored as text:<your text>. Use this for characters and strings, not for physical key remaps.',
    textTabBody:
      'This action inserts text as-is. Good for symbols like €, snippets like "TODO: ", and other printable strings.',
    physicalKeyHint:
      "Letters and symbols here mean the physical key position on a standard US keyboard, not the character produced by the user's current layout.",
    captureKeys: 'Capture',
    assignKey: 'Assign',
    listeningKeys: 'Listening…',
    pressEscapeToStop: 'Press Escape to stop',
    noResults: 'No results',
    chordHint:
      'For a chord, type it manually in the field above. Examples: Ctrl+KeyH, Shift+Space, AltLeft+KeyJ.',
    commandsHint: 'Commands are defined on the Commands tab.',
    macrosHint: 'Macros are defined on the Macros tab.',
    systemMacrosHint:
      'Built-in macros that cannot be edited. To customize, create a user macro based on one.',
    specialHint: 'These are special key codes, not direct system calls.',
    mediaHint:
      'These are media/special keys, not system action calls. If your OS remaps them, the OS remapping may still apply.',
    mouseHint:
      'Extra mouse buttons vary by device. Use the listen button to capture the exact button code.',
    otherHint: 'These are key codes, not direct system action calls.',
  },
  categories: {
    special: 'Special',
    lettersSymbols: 'Letters and symbols',
    digitsNumpad: 'Digits and numpad',
    media: 'Media',
    mouse: 'Mouse',
    other: 'Other',
    commands: 'Commands',
    macros: 'Macros',
    systemMacros: 'System macros',
    app: 'App',
    system: 'System',
    text: 'Text',
  },
  appActions: {
    showQuickMenu: 'Show Quick Action Menu page {n}',
    showEmojiMenu: 'Show Emoji & Symbols menu page {n}',
  },
  systemActions: {
    switchDesktop: 'Switch to desktop {n}',
    switchLayout: 'Switch to layout {n}',
    taskEntry: 'Activate task manager entry {n}',
    walkThroughWindowsAlternative: 'Walk Through Windows Alternative',
    walkThroughWindowsCurrentApp: 'Walk through windows of current application',
    showClipboardHistory: 'Show clipboard history',
    volumeDown: 'Decrease volume',
    volumeUp: 'Increase volume',
    muteAudio: 'Toggle mute',
    brightnessDown: 'Decrease screen brightness',
    brightnessUp: 'Increase screen brightness',
    windowClose: 'Close active window',
    windowToNextDesktop: 'Move window to next desktop',
    windowToPreviousDesktop: 'Move window to previous desktop',
    windowKeepAbove: 'Keep window above others',
    windowMaximizeVertical: 'Maximize window vertically',
    windowMaximizeHorizontal: 'Maximize window horizontally',
    screenOff: 'Turn off screen',
    launchKrunner: 'Launch KRunner',
    launchSystemMonitor: 'Launch System Monitor',
    manageActivities: 'Manage activities',
    nextActivity: 'Next activity',
    previousActivity: 'Previous activity',
    muteMicrophone: 'Mute microphone',
    showDisplayConfig: 'Show display configuration',
    toggleTouchpad: 'Toggle touchpad',
    lockSession: 'Lock session',
    logout: 'Log out',
    logoutWithoutConfirmation: 'Log out without confirmation',
    increaseKeyboardBrightness: 'Increase keyboard brightness',
    decreaseKeyboardBrightness: 'Decrease keyboard brightness',
    toggleKeyboardBacklight: 'Toggle keyboard backlight',
    activateApplicationLauncher: 'Activate application launcher',
    showDesktop: 'Show desktop',
    maximizeWindow: 'Maximize window',
    minimizeWindow: 'Minimize window',
    moveWindow: 'Move window',
    windowToNextScreen: 'Move window to next screen',
    windowToPreviousScreen: 'Move window to previous screen',
    quickTileWindowTop: 'Quick tile window to top',
    quickTileWindowBottom: 'Quick tile window to bottom',
    quickTileWindowLeft: 'Quick tile window to left',
    quickTileWindowRight: 'Quick tile window to right',
    toggleNightColor: 'Toggle night color',
    toggleGridView: 'Toggle grid view',
    toggleOverview: 'Toggle overview',
    togglePresentWindowsAllDesktops: 'Toggle present windows (all desktops)',
    togglePresentWindowsCurrentDesktop:
      'Toggle present windows (current desktop)',
    windowMenu: 'Window operations menu',
    zoomIn: 'Zoom in',
    zoomOut: 'Zoom out',
    zoomActualSize: 'Zoom to actual size',
    killWindow: 'Force close window',
    windowFullscreen: 'Make window fullscreen',
    windowOnAllDesktops: 'Keep window on all desktops',
  },
  mapper: {
    desktopOnly:
      'The mapper is available only in the desktop app. Browser preview cannot access input devices.',
    listFailed: 'Failed to list devices: {err}',
  },
  language: {
    name: 'English',
  },
}
