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
  // Human-readable name (RU) for the dropdown.
  name: string
  // Short implementation hint shown as a secondary label.
  hint?: string
  // Platforms where this function is currently wired up in the mapper.
  platforms: string[]
}

// KDE supports up to 20 virtual desktops, but 10 is a comfortable list
// length for the picker. Bump if needed.
const KDE_DESKTOP_COUNT = 10

export const SYSTEM_ACTIONS: SystemAction[] = [
  ...Array.from({ length: KDE_DESKTOP_COUNT }, (_, i) => {
    const n = i + 1
    return {
      id: `switchDesktop${n}`,
      name: `Переключиться на рабочий стол ${n}`,
      hint: 'KDE: qdbus org.kde.KWin /KWin setCurrentDesktop ' + n,
      platforms: ['linux-kde'],
    }
  }),
]

const BY_ID: Record<string, SystemAction> = Object.fromEntries(
  SYSTEM_ACTIONS.map((a) => [a.id, a]),
)

export function systemActionById(id: string): SystemAction | undefined {
  return BY_ID[id]
}
