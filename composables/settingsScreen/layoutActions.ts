import type { AppConfig, LayoutPreset } from "~/types/config";
import {
  isUserLayoutId,
  userLayoutNameFromId,
} from "~/composables/useLayoutLibrary";
import { normalizeLayoutName, validateLayoutName } from "~/utils/layoutNames";
import {
  emptyLayoutPreset,
  loadBuiltinLayout,
} from "~/utils/layoutPresets";
import { useLayoutApply } from "./useLayoutApply";
import { useLayoutSave } from "./useLayoutSave";
import { useLayoutEdit } from "./useLayoutEdit";
import { useLayoutReset } from "./useLayoutReset";
import { useLayoutDelete } from "./useLayoutDelete";
import { useOverwriteConfirm } from "./useOverwriteConfirm";

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
  const { t } = useI18n();

  function nextAvailableName(baseName: string): string {
    let candidate = baseName;
    let index = 2;
    while (library.layoutExists(candidate)) {
      candidate = `${baseName} ${index}`;
      index += 1;
    }
    return candidate;
  }

  const apply = useLayoutApply({
    library,
    applyPreset,
    replaceCurrentLayoutSnapshot,
  });

  const save = useLayoutSave({
    config,
    currentLayoutId,
    library,
    markLayoutSavedAs,
  });

  const edit = useLayoutEdit({
    config,
    currentLayoutId,
    currentLayoutDescription,
    library,
    markLayoutSavedAs,
  });

  const reset = useLayoutReset({ resetCurrentLayout });

  const del = useLayoutDelete({ config, library, flush });

  const overwriteConfirm = useOverwriteConfirm();

  function validateNameOrSetError(name: string, target: "save" | "edit"): string | null {
    const normalized = normalizeLayoutName(name);
    const code = validateLayoutName(normalized);
    if (!code) return normalized;

    const key =
      code === "empty"
        ? "settings.saveErrorEmpty"
        : "settings.saveErrorInvalidName";
    const message = t(key);
    if (target === "save") save.saveError.value = message;
    else edit.editError.value = message;
    return null;
  }

  async function performSaveWithOverwrite(overwrite = true) {
    const name = validateNameOrSetError(save.saveName.value, "save");
    if (!name) return;

    if (
      !overwrite &&
      library.layoutExists(name) &&
      (!isUserLayoutId(currentLayoutId.value) ||
        userLayoutNameFromId(currentLayoutId.value!) !== name)
    ) {
      overwriteConfirm.requestOverwrite("saveAs", name);
      return;
    }

    await save.performSave(overwrite);
  }

  async function performEditWithOverwrite(overwrite = false) {
    const entry = edit.editPending.value;
    if (!entry) return;
    const oldName = entry.name;
    const newName = validateNameOrSetError(edit.editName.value, "edit");
    if (!newName) return;

    const collides =
      newName !== oldName &&
      library.layoutExists(newName);
    if (collides && !overwrite) {
      overwriteConfirm.requestOverwrite("rename", newName);
      return;
    }

    await edit.performEdit(overwrite);
  }

  async function confirmOverwrite() {
    const action = overwriteConfirm.overwriteAction.value;
    overwriteConfirm.closeOverwriteConfirm();
    if (action === "saveAs") {
      await performSaveWithOverwrite(true);
      return;
    }
    if (action === "rename") {
      await performEditWithOverwrite(true);
    }
  }

  async function createFromEmpty() {
    apply.applying.value = "create:empty";
    apply.applyError.value = null;
    try {
      const preset = emptyLayoutPreset();
      const savedName = await library.saveUserPreset(
        nextAvailableName(t("welcome.defaultEmptyFileName")),
        preset,
        false,
      );
      await replaceCurrentLayoutSnapshot(preset, savedName);
    } catch (error) {
      apply.applyError.value = error instanceof Error ? error.message : String(error);
    } finally {
      apply.applying.value = "";
    }
  }

  async function createFromIvanK() {
    apply.applying.value = "create:ivank";
    apply.applyError.value = null;
    try {
      const preset = await loadBuiltinLayout(t);
      if (!preset) {
        apply.applyError.value = t("welcome.loadError");
        return;
      }
      const savedName = await library.saveUserPreset(
        nextAvailableName(t("welcome.defaultIvanKFileName")),
        preset,
        false,
      );
      await replaceCurrentLayoutSnapshot(preset, savedName);
    } catch (error) {
      apply.applyError.value = error instanceof Error ? error.message : String(error);
    } finally {
      apply.applying.value = "";
    }
  }

  return {
    applying: apply.applying,
    applyError: apply.applyError,
    pendingApply: apply.pendingApply,
    requestApplyEntry: apply.requestApplyEntry,
    cancelApply: apply.cancelApply,
    confirmApply: apply.confirmApply,
    createFromEmpty,
    createFromIvanK,
    saveModalOpen: save.saveModalOpen,
    saveName: save.saveName,
    saveBusy: save.saveBusy,
    saveError: save.saveError,
    saveCurrentLayout: save.saveCurrentLayout,
    openSaveModal: save.openSaveModal,
    openSaveAsModal: save.openSaveAsModal,
    performSave: performSaveWithOverwrite,
    closeSaveModal: save.closeSaveModal,
    editModalOpen: edit.editModalOpen,
    editName: edit.editName,
    editDescription: edit.editDescription,
    editBusy: edit.editBusy,
    editError: edit.editError,
    editPending: edit.editPending,
    openEditModal: edit.openEditModal,
    performEdit: performEditWithOverwrite,
    closeEditModal: edit.closeEditModal,
    overwriteConfirmOpen: overwriteConfirm.overwriteConfirmOpen,
    overwriteTargetName: overwriteConfirm.overwriteTargetName,
    confirmOverwrite,
    closeOverwriteConfirm: overwriteConfirm.closeOverwriteConfirm,
    resetConfirmOpen: reset.resetConfirmOpen,
    resetBusy: reset.resetBusy,
    requestReset: reset.requestReset,
    confirmReset: reset.confirmReset,
    closeResetConfirm: reset.closeResetConfirm,
    deletePending: del.deletePending,
    deleteBusy: del.deleteBusy,
    confirmDelete: del.confirmDelete,
    clearDeletePending: del.clearDeletePending,
  };
}
