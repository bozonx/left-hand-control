interface LayoutResetOptions {
  resetCurrentLayout: () => Promise<void>;
}

export function useLayoutReset({ resetCurrentLayout }: LayoutResetOptions) {
  const resetConfirmOpen = ref(false);
  const resetBusy = ref(false);
  const toast = useToast();

  function requestReset() {
    resetConfirmOpen.value = true;
  }

  async function confirmReset() {
    resetBusy.value = true;
    try {
      await resetCurrentLayout();
      resetConfirmOpen.value = false;
    } catch (error) {
      logger.error('Reset layout failed', error);
      toast.add({
        title: 'Failed to reset layout',
        description: error instanceof Error ? error.message : String(error),
        color: 'error',
        icon: 'i-lucide-circle-alert',
        close: true,
      });
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
