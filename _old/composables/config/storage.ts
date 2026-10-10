export async function readConfigRaw(): Promise<string> {
  const tauri = await useTauri();
  if (!tauri) return "";
  return await tauri.invoke<string>("load_config");
}

export async function writeConfigRaw(contents: string): Promise<void> {
  const tauri = await useTauri();
  if (!tauri) return;
  await tauri.invoke("save_config", { contents });
}

export async function writeUserLayoutRaw(
  name: string,
  contents: string,
  overwrite = true,
): Promise<string> {
  const tauri = await useTauri();
  if (!tauri) return name;
  return await tauri.invoke<string>("save_user_layout", {
    name,
    contents,
    overwrite,
  });
}

export async function getSettingsDir(): Promise<string> {
  const tauri = await useTauri();
  if (!tauri) return "";
  try {
    return await tauri.invoke<string>("get_settings_dir");
  } catch {
    return "";
  }
}

// True when another process (the Slint shell, a text editor) changed
// config.json since it was loaded.
export async function configChangedOnDisk(): Promise<boolean> {
  const tauri = await useTauri();
  if (!tauri) return false;
  return await tauri.invoke<boolean>("config_changed_on_disk");
}

// Saves refused because the file changed on disk carry this marker.
export function isExternalChangeError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.includes("EXTERNAL_CHANGE");
}
