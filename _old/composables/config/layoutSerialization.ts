import type { LayoutPreset } from '~/types/config'

export function clonePreset(preset: LayoutPreset): LayoutPreset {
  return JSON.parse(JSON.stringify(preset))
}
