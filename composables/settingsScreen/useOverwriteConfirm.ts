import type { OverwriteAction } from "./types";

export function useOverwriteConfirm() {
  const overwriteConfirmOpen = ref(false);
  const overwriteAction = ref<OverwriteAction | null>(null);
  const overwriteTargetName = ref("");

  function requestOverwrite(action: OverwriteAction, name: string) {
    overwriteAction.value = action;
    overwriteTargetName.value = name;
    overwriteConfirmOpen.value = true;
  }

  function closeOverwriteConfirm() {
    overwriteConfirmOpen.value = false;
    overwriteAction.value = null;
  }

  return {
    overwriteConfirmOpen,
    overwriteAction,
    overwriteTargetName,
    requestOverwrite,
    closeOverwriteConfirm,
  };
}
