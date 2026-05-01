import type { AppConfig, LayoutPreset } from "~/types/config";
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import type { OverwriteAction, PendingApply } from "./types";
import {
  isUserLayoutId,
  userLayoutId,
  userLayoutNameFromId,
} from "~/composables/useLayoutLibrary";
import { normalizeLayoutName, validateLayoutName } from "~/utils/layoutNames";
import {
  emptyLayoutPreset,
  extractPresetFromConfig,
  loadBuiltinLayout,
} from "~/utils/layoutPresets";

interface LayoutActionOptions {
  config: Ref<AppConfig>;
  currentLayoutId: ComputedRef<string | undefined>;
  currentLayoutDescription: ComputedRef<string>;
  library: ReturnType<typeof useLayoutLibrary>;
  applyPreset: (preset: LayoutPreset, layoutId: string | undefined) => Promise<void>;
  markLayoutSavedAs: (layoutId: string) => Promise<void>;
  replaceCurrentLayoutSnapshot: (preset: LayoutPreset, layoutId: string) => Promise<void>;
  resetCurrentLayout: () => Promise<void>;
  flush: () => Promise<void>;
}

export function useSettingsLayoutActions({
  config,
  currentLayoutId,
  currentLayoutDescription,
  library,
  applyPreset,
  markLayoutSavedAs,
  replaceCurrentLayoutSnapshot,
  resetCurrentLayout,
  flush,
}: LayoutActionOptions) {
  const toast = useToast();
  const { t } = useI18n();

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
      toast.add({
        title: t("settings.layoutApplied", { name: target.entry.name }),
        color: "success",
        icon: "i-lucide-check",
      });
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

  async function saveCurrentLayout() {
    if (isUserLayoutId(currentLayoutId.value)) {
      saveName.value = userLayoutNameFromId(currentLayoutId.value!);
      await performSave(true);
      return;
    }
    openSaveModal();
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

      const oldId = entry.id;
      const newId = userLayoutId(savedName);
      if (oldId !== newId) {
        const order = config.value.settings.layoutOrder;
        const idx = order.indexOf(oldId);
        if (idx !== -1) {
          order[idx] = newId;
        }
        const conditions = config.value.settings.layoutConditions;
        if (conditions[oldId]) {
          conditions[newId] = conditions[oldId];
          delete conditions[oldId];
        }
        if (config.value.settings.autoDefaultLayoutId === oldId) {
          config.value.settings.autoDefaultLayoutId = newId;
        }
        if (config.value.settings.manualActiveLayoutId === oldId) {
          config.value.settings.manualActiveLayoutId = newId;
        }
        if (config.value.settings.currentLayoutId === oldId) {
          config.value.settings.currentLayoutId = newId;
        }
      }

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
      const { settings } = config.value;
      await library.deleteUserPreset(userLayoutNameFromId(entry.id));
      if (settings.currentLayoutId === entry.id) {
        settings.currentLayoutId = undefined;
      }
      if (settings.manualActiveLayoutId === entry.id) {
        settings.manualActiveLayoutId = undefined;
      }
      if (settings.autoDefaultLayoutId === entry.id) {
        settings.autoDefaultLayoutId = undefined;
      }
      settings.layoutOrder = settings.layoutOrder.filter((id) => id !== entry.id);
      delete settings.layoutConditions[entry.id];
      await flush();
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

  return {
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
    saveCurrentLayout,
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
  };
}
