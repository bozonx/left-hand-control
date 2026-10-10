import { type LayoutPreset, USER_LAYOUT_PREFIX } from "~/types/config";
import {
  parseLayoutYaml,
  serializeLayoutYaml,
} from "~/utils/layoutPresets";

export function userLayoutId(name: string): string {
  return `${USER_LAYOUT_PREFIX}${name}`;
}

export function isUserLayoutId(id: string | undefined): boolean {
  return !!id && id.startsWith(USER_LAYOUT_PREFIX);
}

export function userLayoutNameFromId(id: string): string {
  return id.startsWith(USER_LAYOUT_PREFIX)
    ? id.slice(USER_LAYOUT_PREFIX.length)
    : id;
}

export interface LayoutLibraryEntry {
  id: string;
  name: string;
  description?: string;
}

interface LayoutLibraryState {
  entries: Ref<LayoutLibraryEntry[]>;
  layoutsDir: Ref<string>;
  error: Ref<string | null>;
  refresh: () => Promise<void>;
  loadPreset: (id: string) => Promise<LayoutPreset | null>;
  saveUserPreset: (
    name: string,
    preset: LayoutPreset,
    overwrite?: boolean,
  ) => Promise<string>;
  renameUserPreset: (
    oldName: string,
    newName: string,
    preset: LayoutPreset,
    overwrite?: boolean,
  ) => Promise<string>;
  deleteUserPreset: (name: string) => Promise<void>;
  layoutExists: (name: string) => boolean;
}

let singleton: LayoutLibraryState | null = null;

export function resetLayoutLibraryStateForTests() {
  singleton = null;
}

export function useLayoutLibrary(): LayoutLibraryState {
  if (singleton) return singleton;

  const entries = ref<LayoutLibraryEntry[]>([]);
  const layoutsDir = ref("");
  const error = ref<string | null>(null);

  async function refresh() {
    const tauri = await useTauri();
    if (!tauri) {
      entries.value = [];
      layoutsDir.value = "";
      error.value = null;
      return;
    }

    try {
      const names = await tauri.invoke<string[]>("list_user_layouts");
      layoutsDir.value = await tauri.invoke<string>("get_layouts_dir");
      const loaded = await Promise.all(
        names.map(async (name) => {
          try {
            const yaml = await tauri.invoke<string>("load_user_layout", { name });
            const preset = parseLayoutYaml(yaml);
            return {
              id: userLayoutId(name),
              name,
              description: preset?.description,
            } satisfies LayoutLibraryEntry;
          } catch {
            return {
              id: userLayoutId(name),
              name,
            } satisfies LayoutLibraryEntry;
          }
        }),
      );
      entries.value = loaded;
      error.value = null;
    } catch (e) {
      entries.value = [];
      error.value = e instanceof Error ? e.message : String(e);
    }
  }

  async function loadPreset(id: string): Promise<LayoutPreset | null> {
    if (!isUserLayoutId(id)) return null;
    const name = userLayoutNameFromId(id);
    const tauri = await useTauri();
    if (!tauri) return null;
    try {
      const yaml = await tauri.invoke<string>("load_user_layout", { name });
      error.value = null;
      return parseLayoutYaml(yaml);
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
      return null;
    }
  }

  async function saveUserPreset(
    name: string,
    preset: LayoutPreset,
    overwrite = true,
  ): Promise<string> {
    const yaml = serializeLayoutYaml(preset);
    const tauri = await useTauri();
    if (!tauri) return name;
    const savedName = await tauri.invoke<string>("save_user_layout", {
      name,
      contents: yaml,
      overwrite,
    });
    await refresh();
    return savedName;
  }

  async function renameUserPreset(
    oldName: string,
    newName: string,
    preset: LayoutPreset,
    overwrite = false,
  ): Promise<string> {
    const yaml = serializeLayoutYaml(preset);
    const tauri = await useTauri();
    if (!tauri) return newName;
    const savedName = await tauri.invoke<string>("rename_user_layout", {
      oldName,
      newName,
      contents: yaml,
      overwrite,
    });
    await refresh();
    return savedName;
  }

  async function deleteUserPreset(name: string) {
    const tauri = await useTauri();
    if (!tauri) return;
    await tauri.invoke("delete_user_layout", { name });
    await refresh();
  }

  function layoutExists(name: string): boolean {
    return entries.value.some((entry) => entry.name === name);
  }

  singleton = {
    entries,
    layoutsDir,
    error,
    refresh,
    loadPreset,
    saveUserPreset,
    renameUserPreset,
    deleteUserPreset,
    layoutExists,
  };
  return singleton;
}
