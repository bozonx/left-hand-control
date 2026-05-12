export function e2eTarget() {
  return process.env.LHC_E2E_TARGET || 'desktop'
}

export function isKdeWaylandTarget() {
  return e2eTarget() === 'kde-wayland'
}

export function isWindowsTarget() {
  return e2eTarget() === 'windows'
}

export function expectedOsForTarget() {
  if (isWindowsTarget()) return 'windows'
  if (isKdeWaylandTarget()) return 'linux'
  return null
}
