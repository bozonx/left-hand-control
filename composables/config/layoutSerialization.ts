import type { AppConfig, LayoutPreset } from '~/types/config'
import {
  extractPresetFromConfig,
  parseLayoutYaml,
  serializeLayoutYaml,
} from '~/utils/layoutPresets'
export function parseCurrentLayout(raw: string): LayoutPreset | null {
  return parseLayoutYaml(raw)
}

export function serializeCurrentLayout(config: AppConfig): string {
  return serializeLayoutYaml(extractPresetFromConfig(config))
}

export function clonePreset(preset: LayoutPreset): LayoutPreset {
  return JSON.parse(JSON.stringify(preset))
}
