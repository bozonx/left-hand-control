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
      // eslint-disable-next-line no-console
      console.error("Delete layout failed:", error);
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
