import type { AppConfig, LayoutPreset } from "~/types/config";
import {
  extractPresetFromConfig,
  parseLayoutYaml,
  serializeLayoutYaml,
} from "~/utils/layoutPresets";
import { parsePersistedConfig } from "./normalization";

export function parseCurrentLayout(raw: string): LayoutPreset | null {
  return parseLayoutYaml(raw);
}

export function serializeCurrentLayout(config: AppConfig): string {
  return serializeLayoutYaml(extractPresetFromConfig(config));
}

export function legacyPresetFromPersistedConfig(raw: string): LayoutPreset | null {
  try {
    const parsed = parsePersistedConfig(raw);
    return extractPresetFromConfig(parsed);
  } catch {
    return null;
  }
}

export function clonePreset(preset: LayoutPreset): LayoutPreset {
  return JSON.parse(JSON.stringify(preset));
}
