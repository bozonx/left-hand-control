interface LayoutResetOptions {
  resetCurrentLayout: () => Promise<void>;
}

export function useLayoutReset({ resetCurrentLayout }: LayoutResetOptions) {
  const resetConfirmOpen = ref(false);
  const resetBusy = ref(false);
  const resetError = ref<string | null>(null);

  function requestReset() {
    resetConfirmOpen.value = true;
  }

  async function confirmReset() {
    resetBusy.value = true;
    resetError.value = null;
    try {
      await resetCurrentLayout();
      resetConfirmOpen.value = false;
    } catch (error) {
      logger.error('Reset layout failed', error);
      resetError.value = error instanceof Error ? error.message : String(error);
    } finally {
      resetBusy.value = false;
    }
  }

  function closeResetConfirm() {
    resetConfirmOpen.value = false;
    resetError.value = null;
  }

  return {
    resetConfirmOpen,
    resetBusy,
    resetError,
    requestReset,
    confirmReset,
    closeResetConfirm,
  };
}
