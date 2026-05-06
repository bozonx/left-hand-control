import type { AppConfig } from "~/types/config";
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import { userLayoutNameFromId } from "~/composables/useLayoutLibrary";

interface LayoutDeleteOptions {
  config: Ref<AppConfig>;
  library: ReturnType<typeof useLayoutLibrary>;
  flush: () => Promise<void>;
}

export function useLayoutDelete({ config, library, flush }: LayoutDeleteOptions) {
  const deletePending = ref<LayoutLibraryEntry | null>(null);
  const deleteBusy = ref(false);
  const toast = useToast();

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
      settings.layoutOrder = settings.layoutOrder.filter((id) => id !== entry.id);
      delete settings.layoutConditions[entry.id];
      await flush();
      deletePending.value = null;
    } catch (error) {
      logger.error('Delete layout failed', error);
      toast.add({
        title: 'Failed to delete layout',
        description: error instanceof Error ? error.message : String(error),
        color: 'error',
        icon: 'i-lucide-circle-alert',
        close: true,
      });
    } finally {
      deleteBusy.value = false;
    }
  }

  function clearDeletePending() {
    deletePending.value = null;
  }

  return {
    deletePending,
    deleteBusy,
    confirmDelete,
    clearDeletePending,
  };
}
