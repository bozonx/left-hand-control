export function e2eTarget(): string {
  return process.env.LHC_E2E_TARGET || 'desktop'
}

export function isKdeWaylandTarget(): boolean {
  return e2eTarget() === 'kde-wayland'
}

export function isWindowsTarget(): boolean {
  return e2eTarget() === 'windows'
}

export function expectedOsForTarget(): string | null {
  if (isWindowsTarget()) return 'windows'
  if (isKdeWaylandTarget()) return 'linux'
  return null
}
