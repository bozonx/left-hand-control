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
    id: 'walkThroughWindowsAlternative',
    nameKey: 'systemActions.walkThroughWindowsAlternative',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Walk Through Windows Alternative"',
    platforms: ['linux-kde'],
  },
  {
    id: 'walkThroughWindowsCurrentApp',
    nameKey: 'systemActions.walkThroughWindowsCurrentApp',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Walk Through Windows of Current Application"',
    platforms: ['linux-kde'],
  },
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
    id: 'windowToPreviousDesktop',
    nameKey: 'systemActions.windowToPreviousDesktop',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window to Previous Desktop"',
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
    id: 'screenOff',
    nameKey: 'systemActions.screenOff',
    hint: 'KDE: org.kde.kglobalaccel /component/org_kde_powerdevil invokeShortcut "Turn Off Screen"',
    platforms: ['linux-kde'],
  },
  {
    id: 'launchKrunner',
    nameKey: 'systemActions.launchKrunner',
    hint: 'KDE: org.kde.kglobalaccel /component/org_kde_krunner_desktop invokeShortcut _launch',
    platforms: ['linux-kde'],
  },
  {
    id: 'launchSystemMonitor',
    nameKey: 'systemActions.launchSystemMonitor',
    hint: 'KDE: org.kde.kglobalaccel /component/org_kde_plasma_systemmonitor_desktop invokeShortcut _launch',
    platforms: ['linux-kde'],
  },
  {
    id: 'manageActivities',
    nameKey: 'systemActions.manageActivities',
    hint: 'KDE: org.kde.kglobalaccel /component/plasmashell invokeShortcut "manage activities"',
    platforms: ['linux-kde'],
  },
  {
    id: 'nextActivity',
    nameKey: 'systemActions.nextActivity',
    hint: 'KDE: org.kde.kglobalaccel /component/plasmashell invokeShortcut "next activity"',
    platforms: ['linux-kde'],
  },
  {
    id: 'previousActivity',
    nameKey: 'systemActions.previousActivity',
    hint: 'KDE: org.kde.kglobalaccel /component/plasmashell invokeShortcut "previous activity"',
    platforms: ['linux-kde'],
  },
  {
    id: 'muteMicrophone',
    nameKey: 'systemActions.muteMicrophone',
    hint: 'KDE: org.kde.kglobalaccel /component/kmix invokeShortcut mic_mute',
    platforms: ['linux-kde'],
  },
  {
    id: 'showDisplayConfig',
    nameKey: 'systemActions.showDisplayConfig',
    hint: 'KDE: org.kde.kglobalaccel /component/org_kde_kscreen_desktop invokeShortcut ShowOSD',
    platforms: ['linux-kde'],
  },
  {
    id: 'toggleTouchpad',
    nameKey: 'systemActions.toggleTouchpad',
    hint: 'KDE: org.kde.kglobalaccel /component/org_kde_touchpadshortcuts_desktop invokeShortcut ToggleTouchpad',
    platforms: ['linux-kde'],
  },
  {
    id: 'lockSession',
    nameKey: 'systemActions.lockSession',
    hint: 'KDE: org.kde.kglobalaccel /component/ksmserver invokeShortcut "Lock Session"',
    platforms: ['linux-kde'],
  },
  {
    id: 'logout',
    nameKey: 'systemActions.logout',
    hint: 'KDE: org.kde.kglobalaccel /component/ksmserver invokeShortcut "Log Out"',
    platforms: ['linux-kde'],
  },
  {
    id: 'logoutWithoutConfirmation',
    nameKey: 'systemActions.logoutWithoutConfirmation',
    hint: 'KDE: org.kde.kglobalaccel /component/ksmserver invokeShortcut "Log Out Without Confirmation"',
    platforms: ['linux-kde'],
  },
  {
    id: 'increaseKeyboardBrightness',
    nameKey: 'systemActions.increaseKeyboardBrightness',
    hint: 'KDE: org.kde.kglobalaccel /component/org_kde_powerdevil invokeShortcut "Increase Keyboard Brightness"',
    platforms: ['linux-kde'],
  },
  {
    id: 'decreaseKeyboardBrightness',
    nameKey: 'systemActions.decreaseKeyboardBrightness',
    hint: 'KDE: org.kde.kglobalaccel /component/org_kde_powerdevil invokeShortcut "Decrease Keyboard Brightness"',
    platforms: ['linux-kde'],
  },
  {
    id: 'toggleKeyboardBacklight',
    nameKey: 'systemActions.toggleKeyboardBacklight',
    hint: 'KDE: org.kde.kglobalaccel /component/org_kde_powerdevil invokeShortcut "Toggle Keyboard Backlight"',
    platforms: ['linux-kde'],
  },
  {
    id: 'activateApplicationLauncher',
    nameKey: 'systemActions.activateApplicationLauncher',
    hint: 'KDE: org.kde.kglobalaccel /component/plasmashell invokeShortcut "activate application launcher"',
    platforms: ['linux-kde'],
  },
  {
    id: 'showDesktop',
    nameKey: 'systemActions.showDesktop',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Show Desktop"',
    platforms: ['linux-kde'],
  },
  {
    id: 'maximizeWindow',
    nameKey: 'systemActions.maximizeWindow',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Maximize"',
    platforms: ['linux-kde'],
  },
  {
    id: 'minimizeWindow',
    nameKey: 'systemActions.minimizeWindow',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Minimize"',
    platforms: ['linux-kde'],
  },
  {
    id: 'moveWindow',
    nameKey: 'systemActions.moveWindow',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Move"',
    platforms: ['linux-kde'],
  },
  {
    id: 'windowToNextScreen',
    nameKey: 'systemActions.windowToNextScreen',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window to Next Screen"',
    platforms: ['linux-kde'],
  },
  {
    id: 'windowToPreviousScreen',
    nameKey: 'systemActions.windowToPreviousScreen',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window to Previous Screen"',
    platforms: ['linux-kde'],
  },
  {
    id: 'quickTileWindowTop',
    nameKey: 'systemActions.quickTileWindowTop',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Quick Tile Top"',
    platforms: ['linux-kde'],
  },
  {
    id: 'quickTileWindowBottom',
    nameKey: 'systemActions.quickTileWindowBottom',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Quick Tile Bottom"',
    platforms: ['linux-kde'],
  },
  {
    id: 'quickTileWindowLeft',
    nameKey: 'systemActions.quickTileWindowLeft',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Quick Tile Left"',
    platforms: ['linux-kde'],
  },
  {
    id: 'quickTileWindowRight',
    nameKey: 'systemActions.quickTileWindowRight',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Quick Tile Right"',
    platforms: ['linux-kde'],
  },
  {
    id: 'toggleNightColor',
    nameKey: 'systemActions.toggleNightColor',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Toggle Night Color"',
    platforms: ['linux-kde'],
  },
  {
    id: 'toggleGridView',
    nameKey: 'systemActions.toggleGridView',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Grid View"',
    platforms: ['linux-kde'],
  },
  {
    id: 'toggleOverview',
    nameKey: 'systemActions.toggleOverview',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Overview"',
    platforms: ['linux-kde'],
  },
  {
    id: 'togglePresentWindowsAllDesktops',
    nameKey: 'systemActions.togglePresentWindowsAllDesktops',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "ExposeAll"',
    platforms: ['linux-kde'],
  },
  {
    id: 'togglePresentWindowsCurrentDesktop',
    nameKey: 'systemActions.togglePresentWindowsCurrentDesktop',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Expose"',
    platforms: ['linux-kde'],
  },
  {
    id: 'windowMenu',
    nameKey: 'systemActions.windowMenu',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut "Window Operations Menu"',
    platforms: ['linux-kde'],
  },
  {
    id: 'zoomIn',
    nameKey: 'systemActions.zoomIn',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut view_zoom_in',
    platforms: ['linux-kde'],
  },
  {
    id: 'zoomOut',
    nameKey: 'systemActions.zoomOut',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut view_zoom_out',
    platforms: ['linux-kde'],
  },
  {
    id: 'zoomActualSize',
    nameKey: 'systemActions.zoomActualSize',
    hint: 'KDE: org.kde.kglobalaccel /component/kwin invokeShortcut view_actual_size',
    platforms: ['linux-kde'],
  },
]

const BY_ID: Record<string, SystemAction> = Object.fromEntries(
  SYSTEM_ACTIONS.map((a) => [a.id, a]),
)

export function systemActionById(id: string): SystemAction | undefined {
  return BY_ID[id]
}
