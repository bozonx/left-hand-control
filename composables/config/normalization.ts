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
  const rawGameMode =
    raw?.gameMode && typeof raw.gameMode === "object"
      ? raw.gameMode as Partial<AppConfig["settings"]["gameMode"]>
      : {};
  const rawProcessMatchers = rawGameMode.processMatchers as unknown;
  merged.gameMode = {
    ...base.gameMode,
    ...rawGameMode,
    useGamemoded:
      typeof rawGameMode.useGamemoded === "boolean"
        ? rawGameMode.useGamemoded
        : base.gameMode.useGamemoded,
    useFullscreen:
      typeof rawGameMode.useFullscreen === "boolean"
        ? rawGameMode.useFullscreen
        : base.gameMode.useFullscreen,
    processMatchers: Array.isArray(rawProcessMatchers)
      ? rawProcessMatchers
          .filter((item): item is Record<string, unknown> => !!item && typeof item === "object")
          .map((item, index) => ({
            id: typeof item.id === "string" && item.id ? item.id : `process-${index}`,
            name: typeof item.name === "string" ? item.name.trim() : "",
            matchMode: item.matchMode === "exact" ? "exact" as const : "substring" as const,
            onlyActiveWindow: item.onlyActiveWindow !== false,
          }))
          .filter((item) => item.name.length > 0)
      : base.gameMode.processMatchers,
  };
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
  if (!merged.commandTrust || typeof merged.commandTrust !== "object") {
    merged.commandTrust = {};
  } else {
    const cleaned: AppConfig["settings"]["commandTrust"] = {};
    for (const [id, value] of Object.entries(merged.commandTrust)) {
      if (!id || !value || typeof value !== "object") continue;
      const v = value as Partial<AppConfig["settings"]["commandTrust"][string]>;
      if (typeof v.fingerprint !== "string" || !v.fingerprint) continue;
      cleaned[id] = {
        fingerprint: v.fingerprint,
        trustedAt: typeof v.trustedAt === "string" ? v.trustedAt : "",
      };
    }
    merged.commandTrust = cleaned;
  }
  if (
    merged.layoutMode === "manual" &&
    !merged.manualActiveLayoutId &&
    typeof merged.currentLayoutId === "string" &&
    merged.currentLayoutId
  ) {
    merged.manualActiveLayoutId = merged.currentLayoutId;
  }
  if (merged.linuxWaylandTextMode !== 'keycode' && merged.linuxWaylandTextMode !== 'clipboard') {
    merged.linuxWaylandTextMode = undefined;
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
          conditionGameMode:
            rule.conditionGameMode === 'ignore' ||
            rule.conditionGameMode === 'on' ||
            rule.conditionGameMode === 'off'
              ? rule.conditionGameMode
              : undefined,
          conditionLayouts: Array.isArray(rule.conditionLayouts)
            ? rule.conditionLayouts.filter((s): s is string => typeof s === 'string')
            : undefined,
          conditionAppsWhitelist: Array.isArray(rule.conditionAppsWhitelist)
            ? rule.conditionAppsWhitelist.filter((s): s is string => typeof s === 'string')
            : undefined,
          conditionAppsBlacklist: Array.isArray(rule.conditionAppsBlacklist)
            ? rule.conditionAppsBlacklist.filter((s): s is string => typeof s === 'string')
            : undefined,
          isolate: typeof rule.isolate === "string" ? rule.isolate : undefined,
        }))
      : [],
    layerKeymaps:
      r.layerKeymaps && typeof r.layerKeymaps === "object"
        ? r.layerKeymaps
        : base.layerKeymaps,
    macros: Array.isArray(r.macros) ? r.macros : [],
    commands: Array.isArray(r.commands) ? r.commands : [],
    quickActions: Array.isArray(r.quickActions) ? r.quickActions : [],
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
            .filter(
              ([code, value]) =>
                !!code && (value === null || (typeof value === "string" && value.trim() !== "")),
            ),
        );
      }
      if (!Array.isArray(km.extras)) km.extras = [];
    }
  }
  for (const rule of cfg.rules) {
    if (rule.isolate || !rule.layerId) continue;
    const legacyIsolate = cfg.layerKeymaps[rule.layerId]?.isolate;
    if (Array.isArray(legacyIsolate) && legacyIsolate.length > 0) {
      rule.isolate = legacyIsolate.filter((s): s is string => typeof s === "string").join(", ");
    }
  }
  for (const keymap of Object.values(cfg.layerKeymaps)) {
    delete keymap.isolate;
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
