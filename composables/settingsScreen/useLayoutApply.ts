import type { ComputedRef } from "vue";
import type { LayoutPreset } from "~/types/config";
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import {
  userLayoutId,
} from "~/composables/useLayoutLibrary";
import {
  emptyLayoutPreset,
  loadBuiltinLayout,
} from "~/utils/layoutPresets";

interface LayoutApplyOptions {
  library: ReturnType<typeof useLayoutLibrary>;
  applyPreset: (preset: LayoutPreset, layoutId: string | undefined) => Promise<void>;
  replaceCurrentLayoutSnapshot: (preset: LayoutPreset, layoutId: string) => Promise<void>;
  isLayoutDirty: ComputedRef<boolean>;
}

export function useLayoutApply({
  library,
  applyPreset,
  replaceCurrentLayoutSnapshot,
  isLayoutDirty,
}: LayoutApplyOptions) {
  const toast = useToast();
  const { t } = useI18n();

  const applying = ref("");
  const applyError = ref<string | null>(null);
  const pendingApply = ref<{ entry: LayoutLibraryEntry; label: string } | null>(null);

  function nextAvailableName(baseName: string): string {
    let candidate = baseName;
    let index = 2;
    while (library.layoutExists(candidate)) {
      candidate = `${baseName} ${index}`;
      index += 1;
    }
    return candidate;
  }

  function requestApplyEntry(entry: LayoutLibraryEntry) {
    applyError.value = null;
    if (!isLayoutDirty.value) {
      void applyEntry(entry);
      return;
    }
    pendingApply.value = { entry, label: entry.name };
  }

  function cancelApply() {
    pendingApply.value = null;
  }

  async function confirmApply() {
    const target = pendingApply.value;
    if (!target) return;
    await applyEntry(target.entry);
  }

  async function applyEntry(entry: LayoutLibraryEntry) {
    applying.value = entry.id;
    try {
      const preset = await library.loadPreset(entry.id);
      if (!preset) {
        applyError.value = t("settings.loadFailed", { name: entry.name });
        return;
      }
      await applyPreset(preset, entry.id);
      toast.add({
        title: t("settings.layoutApplied", { name: entry.name }),
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

  return {
    applying,
    applyError,
    pendingApply,
    requestApplyEntry,
    cancelApply,
    confirmApply,
    createFromEmpty,
    createFromIvanK,
  };
}
