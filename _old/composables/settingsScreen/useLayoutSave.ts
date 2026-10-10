import type { AppConfig } from "~/types/config";
import {
  isUserLayoutId,
  userLayoutId,
  userLayoutNameFromId,
} from "~/composables/useLayoutLibrary";
import { normalizeLayoutName, validateLayoutName } from "~/utils/layoutNames";
import { extractPresetFromConfig } from "~/utils/layoutPresets";
import { useOverwriteConfirm } from "./useOverwriteConfirm";

interface LayoutSaveOptions {
  config: Ref<AppConfig>;
  currentLayoutId: ComputedRef<string | undefined>;
  library: ReturnType<typeof useLayoutLibrary>;
  markLayoutSavedAs: (layoutId: string) => Promise<void>;
}

export function useLayoutSave({
  config,
  currentLayoutId,
  library,
  markLayoutSavedAs,
}: LayoutSaveOptions) {
  const { t } = useI18n();

  const saveModalOpen = ref(false);
  const saveName = ref("");
  const saveBusy = ref(false);
  const saveError = ref<string | null>(null);

  const overwrite = useOverwriteConfirm();

  function validateSaveName(name: string): string | null {
    const normalized = normalizeLayoutName(name);
    const code = validateLayoutName(normalized);
    if (!code) return normalized;
    saveError.value = t(
      code === "empty" ? "settings.saveErrorEmpty" : "settings.saveErrorInvalidName",
    );
    return null;
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

  async function performSave(overwriteAllowed = true) {
    const name = validateSaveName(saveName.value);
    if (!name) return;

    if (
      !overwriteAllowed &&
      library.layoutExists(name) &&
      (!isUserLayoutId(currentLayoutId.value) ||
        userLayoutNameFromId(currentLayoutId.value!) !== name)
    ) {
      overwrite.requestOverwrite("saveAs", name);
      return;
    }

    saveBusy.value = true;
    saveError.value = null;
    try {
      config.value.layoutDescription = config.value.layoutDescription?.trim() || undefined;
      const preset = extractPresetFromConfig(config.value);
      const savedName = await library.saveUserPreset(name, preset, overwriteAllowed);
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

  async function confirmOverwrite() {
    if (overwrite.overwriteAction.value === "saveAs") {
      overwrite.closeOverwriteConfirm();
      await performSave(true);
    }
  }

  return {
    saveModalOpen,
    saveName,
    saveBusy,
    saveError,
    saveCurrentLayout,
    openSaveModal,
    openSaveAsModal,
    performSave,
    closeSaveModal,
    confirmOverwrite,
    overwriteConfirmOpen: overwrite.overwriteConfirmOpen,
    overwriteAction: overwrite.overwriteAction,
    overwriteTargetName: overwrite.overwriteTargetName,
    closeOverwriteConfirm: overwrite.closeOverwriteConfirm,
  };
}
