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

export async function readCurrentLayoutRaw(): Promise<string> {
  const tauri = await useTauri();
  if (!tauri) return "";
  return await tauri.invoke<string>("load_current_layout");
}

export async function writeCurrentLayoutRaw(contents: string): Promise<void> {
  const tauri = await useTauri();
  if (!tauri) return;
  await tauri.invoke("save_current_layout", { contents });
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
