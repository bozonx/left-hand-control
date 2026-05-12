export function e2eTarget(): string {
  return process.env.LHC_E2E_TARGET || 'desktop'
}

export function isKdeWaylandTarget(): boolean {
  const t = e2eTarget()
  return t === 'kde-wayland' || t === 'linux-kde-wayland'
}

export function isGnomeWaylandTarget(): boolean {
  return e2eTarget() === 'linux-gnome-wayland'
}

export function isSwayWaylandTarget(): boolean {
  return e2eTarget() === 'linux-sway-wayland'
}

export function isWindowsTarget(): boolean {
  return e2eTarget() === 'windows'
}

export function isLinuxWaylandTarget(): boolean {
  return isKdeWaylandTarget() || isGnomeWaylandTarget() || isSwayWaylandTarget()
}

export function expectedOsForTarget(): string | null {
  if (isWindowsTarget()) return 'windows'
  if (isLinuxWaylandTarget()) return 'linux'
  return null
}

export function expectedDesktopForTarget(): string | null {
  if (isKdeWaylandTarget()) return 'KDE Plasma'
  if (isGnomeWaylandTarget()) return 'GNOME'
  if (isSwayWaylandTarget()) return 'Sway'
  return null
}
