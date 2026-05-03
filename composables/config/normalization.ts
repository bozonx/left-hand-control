import type { AppConfig, PersistedConfig } from "~/types/config";
import {
  createDefaultConfig,
  createDefaultPersistedConfig,
} from "~/types/config";

function normalizeConditionSet(
  raw: unknown,
): { gameMode?: "on" | "off"; layouts: string[]; apps?: string[] } | undefined {
  if (!raw || typeof raw !== "object") return undefined;
  const v = raw as { gameMode?: unknown; layouts?: unknown; apps?: unknown };
  const gameMode =
    v.gameMode === "on" || v.gameMode === "off" ? v.gameMode : undefined;
  const layouts = Array.isArray(v.layouts)
    ? v.layouts.filter((s): s is string => typeof s === "string" && !!s)
    : [];
  const apps = Array.isArray(v.apps)
    ? v.apps.filter((s): s is string => typeof s === "string" && !!s.trim())
    : [];
  if (!gameMode && layouts.length === 0 && apps.length === 0) return undefined;
  return {
    gameMode,
    layouts,
    ...(apps.length > 0 ? { apps } : {}),
  };
}

function normalizeSettings(
  base: AppConfig["settings"],
  raw: Partial<AppConfig["settings"]> | undefined,
): AppConfig["settings"] {
  const merged = { ...base, ...(raw ?? {}) };
  merged.layoutMode = merged.layoutMode === "auto" ? "auto" : "manual";
  if (typeof merged.manualActiveLayoutId !== "string" || !merged.manualActiveLayoutId) {
    merged.manualActiveLayoutId = undefined;
  }
  merged.layoutOrder = Array.isArray(merged.layoutOrder)
    ? merged.layoutOrder.filter((id): id is string => typeof id === "string")
    : [];
  if (!merged.layoutConditions || typeof merged.layoutConditions !== "object") {
    merged.layoutConditions = {};
  } else {
    const cleaned: AppConfig["settings"]["layoutConditions"] = {};
    for (const [id, value] of Object.entries(merged.layoutConditions)) {
      if (!value || typeof value !== "object") continue;
      const v = value as Partial<AppConfig["settings"]["layoutConditions"][string]>;
      const rule: AppConfig["settings"]["layoutConditions"][string] = {
        whitelist: normalizeConditionSet(v.whitelist),
        blacklist: normalizeConditionSet(v.blacklist),
      };
      if (v.enabledInAuto === true) rule.enabledInAuto = true;
      if (rule.enabledInAuto || rule.whitelist || rule.blacklist) cleaned[id] = rule;
    }
    merged.layoutConditions = cleaned;
  }
  if (
    merged.layoutMode === "manual" &&
    !merged.manualActiveLayoutId &&
    typeof merged.currentLayoutId === "string" &&
    merged.currentLayoutId
  ) {
    merged.manualActiveLayoutId = merged.currentLayoutId;
  }
  return merged;
}

export function normalizeConfig(raw: unknown): AppConfig {
  const base = createDefaultConfig();
  if (!raw || typeof raw !== "object") return base;
  const r = raw as Partial<AppConfig>;
  const cfg: AppConfig = {
    version: 1,
    layers:
      Array.isArray(r.layers) && r.layers.length
        ? r.layers
        : base.layers,
    rules: Array.isArray(r.rules)
      ? r.rules.map((rule) => ({
          ...rule,
          doubleTapAction: rule.doubleTapAction ?? "",
          tapAction: rule.tapAction ?? "",
          holdAction: rule.holdAction ?? "",
        }))
      : [],
    layerKeymaps:
      r.layerKeymaps && typeof r.layerKeymaps === "object"
        ? r.layerKeymaps
        : base.layerKeymaps,
    macros: Array.isArray(r.macros) ? r.macros : [],
    commands: Array.isArray(r.commands) ? r.commands : [],
    settings: normalizeSettings(base.settings, r.settings),
  };
  for (const layer of cfg.layers) {
    if (!cfg.layerKeymaps[layer.id]) {
      cfg.layerKeymaps[layer.id] = { keys: {}, extras: [] };
    } else {
      const km = cfg.layerKeymaps[layer.id]!;
      if (!km.keys || typeof km.keys !== "object") {
        km.keys = {};
      } else {
        km.keys = Object.fromEntries(
          Object.entries(km.keys)
            .filter(([code, value]) => !!code && (typeof value === "string" || value === null))
            .map(([code, value]) => [code, value === "" ? null : value]),
        );
      }
      if (!Array.isArray(km.extras)) km.extras = [];
    }
  }
  return cfg;
}

export function parsePersistedConfig(raw: string): AppConfig {
  try {
    return normalizeConfig(JSON.parse(raw));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`config.json is invalid: ${message}`);
  }
}

function normalizePersistedConfig(raw: unknown): PersistedConfig {
  const base = createDefaultPersistedConfig();
  if (!raw || typeof raw !== "object") return base;
  const value = raw as Partial<PersistedConfig>;
  return {
    version: 1,
    settings: normalizeSettings(base.settings, value.settings),
  };
}

export function parsePersistedSettings(raw: string): PersistedConfig {
  try {
    return normalizePersistedConfig(JSON.parse(raw));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`config.json is invalid: ${message}`);
  }
}

export function serializePersistedSettings(config: AppConfig): string {
  const persisted: PersistedConfig = {
    version: 1,
    settings: config.settings,
  };
  return JSON.stringify(persisted, null, 2);
}
