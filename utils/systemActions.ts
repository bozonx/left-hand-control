// Catalog of cross-platform "system functions" the mapper can invoke.
//
// Each entry has a stable `id` (referenced from config as `sys:<id>`).
// The Rust side (see `src-tauri/src/mapper/system.rs`) translates that id
// into a concrete shell command for the current OS / desktop environment.
// Some functions are only available on some platforms — the UI still
// allows selecting them so a config is portable across machines, but the
// mapper logs a warning and skips the action at runtime if unsupported.

export interface SystemAction {
  // Stable cross-platform identifier used in config as `sys:<id>`.
  id: string
  // i18n key used to render the human-readable name. May contain named
  // parameters (see `nameParams`).
  nameKey: string
  nameParams?: Record<string, string | number>
  // Short implementation hint shown as a secondary label (not localized —
  // it is a verbatim shell command).
  hint?: string
  // Platforms where this function is currently wired up in the mapper.
  platforms: string[]
}

// KDE supports up to 20 virtual desktops, but 10 is a comfortable list
// length for the picker. Bump if needed.
const KDE_DESKTOP_COUNT = 10
const KDE_LAYOUT_COUNT = 10
const KDE_TASK_ENTRY_COUNT = 10

export const SYSTEM_ACTIONS: SystemAction[] = [
  ...Array.from({ length: KDE_DESKTOP_COUNT }, (_, i) => {
    const n = i + 1
    return {
      id: `switchDesktop${n}`,
      nameKey: 'systemActions.switchDesktop',
      nameParams: { n },
      hint: 'KDE: qdbus org.kde.KWin /KWin setCurrentDesktop ' + n,
      platforms: ['linux-kde'],
    }
  }),
  ...Array.from({ length: KDE_LAYOUT_COUNT }, (_, i) => {
    const n = i + 1
    return {
      id: `switchLayout${n}`,
      nameKey: 'systemActions.switchLayout',
      nameParams: { n },
      hint: 'KDE: qdbus org.kde.keyboard /Layouts org.kde.KeyboardLayouts.setLayout ' + n,
      platforms: ['linux-kde'],
    }
  }),
  ...Array.from({ length: KDE_TASK_ENTRY_COUNT }, (_, i) => {
    const n = i + 1
    return {
      id: `taskEntry${n}`,
      nameKey: 'systemActions.taskEntry',
      nameParams: { n },
      hint: 'KDE: org.kde.kglobalaccel /component/plasmashell invokeShortcut "activate task manager entry ' + n + '"',
      platforms: ['linux-kde'],
    }
  }),
  {
    id: 'showClipboardHistory',
    nameKey: 'systemActions.showClipboardHistory',
    hint: 'KDE: qdbus org.kde.plasmashell /klipper org.kde.klipper.klipper.showKlipperPopupMenu',
    platforms: ['linux-kde'],
  },
  {
    id: 'volumeDown',
    nameKey: 'systemActions.volumeDown',
    hint: 'KDE: org.kde.kglobalaccel /component/kmix invokeShortcut decrease_volume',
    platforms: ['linux-kde'],
  },
  {
    id: 'volumeUp',
    nameKey: 'systemActions.volumeUp',
    hint: 'KDE: org.kde.kglobalaccel /component/kmix invokeShortcut increase_volume',
    platforms: ['linux-kde'],
  },
  {
    id: 'muteAudio',
    nameKey: 'systemActions.muteAudio',
    hint: 'KDE: org.kde.kglobalaccel /component/kmix invokeShortcut mute',
    platforms: ['linux-kde'],
  },
  {
    id: 'brightnessDown',
    nameKey: 'systemActions.brightnessDown',
    hint: 'KDE: org.kde.kglobalaccel /component/org_kde_powerdevil invokeShortcut "Decrease Screen Brightness"',
    platforms: ['linux-kde'],
  },
  {
    id: 'brightnessUp',
    nameKey: 'systemActions.brightnessUp',
    hint: 'KDE: org.kde.kglobalaccel /component/org_kde_powerdevil invokeShortcut "Increase Screen Brightness"',
    platforms: ['linux-kde'],
  },
  {
    id: 'windowClose',
    nameKey: 'systemActions.windowClose',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Close"',
    platforms: ['linux-kde'],
  },
  {
    id: 'windowToNextDesktop',
    nameKey: 'systemActions.windowToNextDesktop',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window to Next Desktop"',
    platforms: ['linux-kde'],
  },
  {
    id: 'windowKeepAbove',
    nameKey: 'systemActions.windowKeepAbove',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Above Other Windows"',
    platforms: ['linux-kde'],
  },
  {
    id: 'windowMaximizeVertical',
    nameKey: 'systemActions.windowMaximizeVertical',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Maximize Vertical"',
    platforms: ['linux-kde'],
  },
  {
    id: 'windowMaximizeHorizontal',
    nameKey: 'systemActions.windowMaximizeHorizontal',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Maximize Horizontal"',
    platforms: ['linux-kde'],
  },
  {
    id: 'screenshot',
    nameKey: 'systemActions.screenshot',
    hint: 'KDE: spectacle -r',
    platforms: ['linux-kde'],
  },
  {
    id: 'screenOff',
    nameKey: 'systemActions.screenOff',
    hint: 'KDE: org.kde.kglobalaccel /component/org_kde_powerdevil invokeShortcut "Turn Off Screen"',
    platforms: ['linux-kde'],
  },
]

const BY_ID: Record<string, SystemAction> = Object.fromEntries(
  SYSTEM_ACTIONS.map((a) => [a.id, a]),
)

export function systemActionById(id: string): SystemAction | undefined {
  return BY_ID[id]
}
