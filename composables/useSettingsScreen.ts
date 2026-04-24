import {
  type LayoutLibraryEntry,
  isUserLayoutId,
  userLayoutId,
  userLayoutNameFromId,
} from "~/composables/useLayoutLibrary";
import { localeDisplayName } from "~/i18n";
import { usePlatformInfo } from "~/composables/usePlatformInfo";
import { normalizeLayoutName, validateLayoutName } from "~/utils/layoutNames";
import {
  emptyLayoutPreset,
  extractPresetFromConfig,
  loadBuiltinLayout,
} from "~/utils/layoutPresets";

interface PendingApply {
  entry: LayoutLibraryEntry;
  label: string;
}

interface SaveDraft {
  name: string;
  description: string;
}

type OverwriteAction = "saveAs" | "rename";

let singleton: ReturnType<typeof useSettingsScreen> | null = null;

export function useSettingsScreen() {
  if (singleton) return singleton;

  const {
    config,
    settingsDir,
    flush,
    applyPreset,
    markLayoutSavedAs,
    replaceCurrentLayoutSnapshot,
    resetCurrentLayout,
    currentLayoutId,
    isLayoutDirty,
  } = useConfig();
  const library = useLayoutLibrary();
  const mapper = useMapper();
  const platform = usePlatformInfo();
  const theme = useAppTheme();
  const appLocale = useAppLocale();
  const { t } = useI18n();

  const appearanceItems = computed(() => [
    { label: t("settings.appearanceItems.system"), value: "system" as const },
    { label: t("settings.appearanceItems.light"), value: "light" as const },
    { label: t("settings.appearanceItems.dark"), value: "dark" as const },
  ]);

  const localeItems = computed(() => {
    const resolvedName = localeDisplayName(appLocale.systemLocale.value);
    return [
      {
        label: t("settings.languageAutoResolved", { resolved: resolvedName }),
        value: "auto" as const,
      },
      ...appLocale.available.map((loc) => ({
        label: localeDisplayName(loc),
        value: loc,
      })),
    ];
  });

  const applying = ref("");
  const applyError = ref<string | null>(null);
  const pendingApply = ref<PendingApply | null>(null);

  const saveModalOpen = ref(false);
  const saveName = ref("");
  const saveBusy = ref(false);
  const saveError = ref<string | null>(null);

  const editModalOpen = ref(false);
  const editName = ref("");
  const editDescription = ref("");
  const editBusy = ref(false);
  const editError = ref<string | null>(null);
  const editPending = ref<LayoutLibraryEntry | null>(null);

  const overwriteConfirmOpen = ref(false);
  const overwriteAction = ref<OverwriteAction | null>(null);
  const overwriteTargetName = ref("");

  const resetConfirmOpen = ref(false);
  const resetBusy = ref(false);

  const deletePending = ref<LayoutLibraryEntry | null>(null);
  const deleteBusy = ref(false);

  const deviceOptions = computed(() =>
    mapper.devices.value.map((device) => ({
      label: `${device.name}  —  ${device.path}`,
      value: device.path,
    })),
  );

  const selectedDevice = computed<string>({
    get: () => config.value.settings.inputDevicePath ?? "",
    set: (value: string) => {
      config.value.settings.inputDevicePath = value;
    },
  });

  const currentLayoutDescription = computed(() => config.value.layoutDescription ?? "");

  function nextAvailableName(baseName: string): string {
    let candidate = baseName;
    let index = 2;
    while (library.layoutExists(candidate)) {
      candidate = `${baseName} ${index}`;
      index += 1;
    }
    return candidate;
  }

  function validateNameOrSetError(name: string, target: "save" | "edit"): string | null {
    const normalized = normalizeLayoutName(name);
    const code = validateLayoutName(normalized);
    if (!code) return normalized;

    const key =
      code === "empty"
        ? "settings.saveErrorEmpty"
        : "settings.saveErrorInvalidName";
    const message = t(key);
    if (target === "save") saveError.value = message;
    else editError.value = message;
    return null;
  }

  function requestApply(entry: LayoutLibraryEntry) {
    applyError.value = null;
    pendingApply.value = {
      entry,
      label: entry.name,
    };
  }

  function cancelApply() {
    pendingApply.value = null;
  }

  async function confirmApply() {
    const target = pendingApply.value;
    if (!target) return;
    applying.value = target.entry.id;
    try {
      const preset = await library.loadPreset(target.entry.id);
      if (!preset) {
        applyError.value = t("settings.loadFailed", { name: target.entry.name });
        return;
      }
      await applyPreset(preset, target.entry.id);
      pendingApply.value = null;
    } catch (error) {
      applyError.value = error instanceof Error ? error.message : String(error);
    } finally {
      applying.value = "";
    }
  }

  async function createFromEmpty() {
    applying.value = "create:empty";
    applyError.value = null;
    try {
      const preset = emptyLayoutPreset();
      const savedName = await library.saveUserPreset(
        nextAvailableName(t("welcome.defaultEmptyFileName")),
        preset,
        false,
      );
      await replaceCurrentLayoutSnapshot(preset, userLayoutId(savedName));
    } catch (error) {
      applyError.value = error instanceof Error ? error.message : String(error);
    } finally {
      applying.value = "";
    }
  }

  async function createFromIvanK() {
    applying.value = "create:ivank";
    applyError.value = null;
    try {
      const preset = await loadBuiltinLayout(t);
      if (!preset) {
        applyError.value = t("welcome.loadError");
        return;
      }
      const savedName = await library.saveUserPreset(
        nextAvailableName(t("welcome.defaultIvanKFileName")),
        preset,
        false,
      );
      await replaceCurrentLayoutSnapshot(preset, userLayoutId(savedName));
    } catch (error) {
      applyError.value = error instanceof Error ? error.message : String(error);
    } finally {
      applying.value = "";
    }
  }

  function openSaveModal() {
    saveError.value = null;
    saveName.value = isUserLayoutId(currentLayoutId.value)
      ? userLayoutNameFromId(currentLayoutId.value!)
      : "";
    saveModalOpen.value = true;
  }

  function openSaveAsModal() {
    saveError.value = null;
    const baseName = isUserLayoutId(currentLayoutId.value)
      ? userLayoutNameFromId(currentLayoutId.value!)
      : t("welcome.defaultEmptyFileName");
    saveName.value = `${baseName} copy`;
    saveModalOpen.value = true;
  }

  async function performSave(overwrite = true) {
    const name = validateNameOrSetError(saveName.value, "save");
    if (!name) return;

    if (
      !overwrite &&
      library.layoutExists(name) &&
      (!isUserLayoutId(currentLayoutId.value) ||
        userLayoutNameFromId(currentLayoutId.value!) !== name)
    ) {
      overwriteAction.value = "saveAs";
      overwriteTargetName.value = name;
      overwriteConfirmOpen.value = true;
      return;
    }

    saveBusy.value = true;
    saveError.value = null;
    try {
      config.value.layoutDescription = config.value.layoutDescription?.trim() || undefined;
      const preset = extractPresetFromConfig(config.value);
      const savedName = await library.saveUserPreset(name, preset, overwrite);
      await markLayoutSavedAs(userLayoutId(savedName));
      saveModalOpen.value = false;
    } catch (error) {
      saveError.value = error instanceof Error ? error.message : String(error);
    } finally {
      saveBusy.value = false;
    }
  }

  function closeSaveModal() {
    saveModalOpen.value = false;
  }

  function openEditModal(entry: LayoutLibraryEntry) {
    editPending.value = entry;
    editName.value = entry.name;
    editDescription.value =
      currentLayoutId.value === entry.id
        ? currentLayoutDescription.value
        : entry.description ?? "";
    editError.value = null;
    editModalOpen.value = true;
  }

  async function performEdit(overwrite = false) {
    const entry = editPending.value;
    if (!entry) return;
    const oldName = entry.name;
    const newName = validateNameOrSetError(editName.value, "edit");
    if (!newName) return;

    const collides =
      newName !== oldName &&
      library.layoutExists(newName);
    if (collides && !overwrite) {
      overwriteAction.value = "rename";
      overwriteTargetName.value = newName;
      overwriteConfirmOpen.value = true;
      return;
    }

    editBusy.value = true;
    editError.value = null;
    try {
      const previousDescription = config.value.layoutDescription;
      const isCurrent = currentLayoutId.value === entry.id;
      const loadedPreset = isCurrent ? null : await library.loadPreset(entry.id);
      const preset = isCurrent
        ? extractPresetFromConfig({
            ...config.value,
            layoutDescription: editDescription.value.trim() || undefined,
          })
        : loadedPreset
          ? {
              ...loadedPreset,
              description: editDescription.value.trim() || undefined,
            }
          : null;

      if (!preset) {
        editError.value = t("settings.loadFailed", { name: entry.name });
        return;
      }

      const savedName = await library.renameUserPreset(
        oldName,
        newName,
        preset,
        overwrite,
      );

      if (isCurrent) {
        config.value.layoutDescription = editDescription.value.trim() || undefined;
        await markLayoutSavedAs(userLayoutId(savedName));
      } else {
        config.value.layoutDescription = previousDescription;
      }

      editModalOpen.value = false;
      editPending.value = null;
    } catch (error) {
      editError.value = error instanceof Error ? error.message : String(error);
    } finally {
      editBusy.value = false;
    }
  }

  function closeEditModal() {
    editModalOpen.value = false;
    editPending.value = null;
  }

  function requestReset() {
    resetConfirmOpen.value = true;
  }

  async function confirmReset() {
    resetBusy.value = true;
    try {
      await resetCurrentLayout();
      resetConfirmOpen.value = false;
    } catch (error) {
      applyError.value = error instanceof Error ? error.message : String(error);
    } finally {
      resetBusy.value = false;
    }
  }

  function closeResetConfirm() {
    resetConfirmOpen.value = false;
  }

  async function confirmOverwrite() {
    const action = overwriteAction.value;
    overwriteConfirmOpen.value = false;
    overwriteAction.value = null;
    if (action === "saveAs") {
      await performSave(true);
      return;
    }
    if (action === "rename") {
      await performEdit(true);
    }
  }

  function closeOverwriteConfirm() {
    overwriteConfirmOpen.value = false;
    overwriteAction.value = null;
  }

  async function confirmDelete() {
    const entry = deletePending.value;
    if (!entry) return;
    deleteBusy.value = true;
    try {
      await library.deleteUserPreset(userLayoutNameFromId(entry.id));
      if (currentLayoutId.value === entry.id) {
        config.value.settings.currentLayoutId = undefined;
        await flush();
      }
      deletePending.value = null;
    } catch (error) {
      applyError.value = error instanceof Error ? error.message : String(error);
    } finally {
      deleteBusy.value = false;
    }
  }

  function clearDeletePending() {
    deletePending.value = null;
  }

  async function toggleMapper() {
    try {
      await flush();
      if (mapper.status.value.running) {
        await mapper.stop();
        return;
      }
      if (!selectedDevice.value) return;
      await mapper.start(selectedDevice.value);
    } catch (error) {
      mapper.error.value =
        error instanceof Error ? error.message : String(error);
    }
  }

  onMounted(async () => {
    await Promise.all([
      mapper.refreshDevices(),
      mapper.refreshStatus(),
      platform.refresh(),
      library.refresh(),
    ]);
  });

  singleton = {
    config,
    settingsDir,
    currentLayoutId,
    currentLayoutDescription,
    isLayoutDirty,
    library,
    mapper,
    platform,
    theme,
    appLocale,
    appearanceItems,
    localeItems,
    applying,
    applyError,
    pendingApply,
    requestApplyEntry: requestApply,
    cancelApply,
    confirmApply,
    createFromEmpty,
    createFromIvanK,
    saveModalOpen,
    saveName,
    saveBusy,
    saveError,
    openSaveModal,
    openSaveAsModal,
    performSave,
    closeSaveModal,
    editModalOpen,
    editName,
    editDescription,
    editBusy,
    editError,
    editPending,
    openEditModal,
    performEdit,
    closeEditModal,
    overwriteConfirmOpen,
    overwriteTargetName,
    confirmOverwrite,
    closeOverwriteConfirm,
    resetConfirmOpen,
    resetBusy,
    requestReset,
    confirmReset,
    closeResetConfirm,
    deletePending,
    deleteBusy,
    confirmDelete,
    clearDeletePending,
    deviceOptions,
    selectedDevice,
    toggleMapper,
  };
  return singleton;
}

