interface LayoutResetOptions {
  resetCurrentLayout: () => Promise<void>;
}

export function useLayoutReset({ resetCurrentLayout }: LayoutResetOptions) {
  const resetConfirmOpen = ref(false);
  const resetBusy = ref(false);

  function requestReset() {
    resetConfirmOpen.value = true;
  }

  async function confirmReset() {
    resetBusy.value = true;
    try {
      await resetCurrentLayout();
      resetConfirmOpen.value = false;
    } catch (error) {
      // eslint-disable-next-line no-console
      console.error("Reset layout failed:", error);
    } finally {
      resetBusy.value = false;
    }
  }

  function closeResetConfirm() {
    resetConfirmOpen.value = false;
  }

  return {
    resetConfirmOpen,
    resetBusy,
    requestReset,
    confirmReset,
    closeResetConfirm,
  };
}
