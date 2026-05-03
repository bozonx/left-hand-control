import type { AppConfig, LayoutPreset } from "~/types/config";
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import {
  userLayoutId,
} from "~/composables/useLayoutLibrary";
import { normalizeLayoutName, validateLayoutName } from "~/utils/layoutNames";
import { extractPresetFromConfig } from "~/utils/layoutPresets";
import { useOverwriteConfirm } from "./useOverwriteConfirm";

interface LayoutEditOptions {
  config: Ref<AppConfig>;
  currentLayoutId: ComputedRef<string | undefined>;
  currentLayoutDescription: ComputedRef<string>;
  library: ReturnType<typeof useLayoutLibrary>;
  markLayoutSavedAs: (layoutId: string) => Promise<void>;
}

export type LayoutEditMode = "full" | "name" | "description";

export function useLayoutEdit({
  config,
  currentLayoutId,
  currentLayoutDescription,
  library,
  markLayoutSavedAs,
}: LayoutEditOptions) {
  const { t } = useI18n();

  const editModalOpen = ref(false);
  const editName = ref("");
  const editDescription = ref("");
  const editMode = ref<LayoutEditMode>("full");
  const editBusy = ref(false);
  const editError = ref<string | null>(null);
  const editPending = ref<LayoutLibraryEntry | null>(null);

  const overwrite = useOverwriteConfirm();

  function validateEditName(name: string): string | null {
    const normalized = normalizeLayoutName(name);
    const code = validateLayoutName(normalized);
    if (!code) return normalized;
    editError.value = t(
      code === "empty" ? "settings.saveErrorEmpty" : "settings.saveErrorInvalidName",
    );
    return null;
  }

  function openEditModal(entry: LayoutLibraryEntry, mode: LayoutEditMode = "full") {
    editPending.value = entry;
    editMode.value = mode;
    editName.value = entry.name;
    editDescription.value =
      currentLayoutId.value === entry.id
        ? currentLayoutDescription.value
        : entry.description ?? "";
    editError.value = null;
    editModalOpen.value = true;
  }

  async function performEdit(overwriteAllowed = false) {
    const entry = editPending.value;
    if (!entry) return;
    const oldName = entry.name;
    const newName = validateEditName(editName.value);
    if (!newName) return;

    const collides = newName !== oldName && library.layoutExists(newName);
    if (collides && !overwriteAllowed) {
      overwrite.requestOverwrite("rename", newName);
      return;
    }

    editBusy.value = true;
    editError.value = null;
    try {
      const previousDescription = config.value.layoutDescription;
      const isCurrent = currentLayoutId.value === entry.id;
      const loadedPreset = isCurrent ? null : await library.loadPreset(entry.id);
      const preset: LayoutPreset | null = isCurrent
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
        overwriteAllowed,
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
    editMode.value = "full";
  }

  async function confirmOverwrite() {
    if (overwrite.overwriteAction.value === "rename") {
      overwrite.closeOverwriteConfirm();
      await performEdit(true);
    }
  }

  return {
    editModalOpen,
    editName,
    editDescription,
    editMode,
    editBusy,
    editError,
    editPending,
    openEditModal,
    performEdit,
    closeEditModal,
    confirmOverwrite,
    overwriteConfirmOpen: overwrite.overwriteConfirmOpen,
    overwriteAction: overwrite.overwriteAction,
    overwriteTargetName: overwrite.overwriteTargetName,
    closeOverwriteConfirm: overwrite.closeOverwriteConfirm,
  };
}
