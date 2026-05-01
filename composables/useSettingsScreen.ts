import {
  type LayoutLibraryEntry,
  isUserLayoutId,
  userLayoutId,
  userLayoutNameFromId,
} from "~/composables/useLayoutLibrary";
import { localeDisplayName } from "~/i18n";
import { usePlatformInfo } from "~/composables/usePlatformInfo";
import { normalizeLayoutName, validateLayoutName } from "~/utils/layoutNames";
import {
  emptyLayoutPreset,
  extractPresetFromConfig,
  loadBuiltinLayout,
} from "~/utils/layoutPresets";
import type { CapabilityStatus } from "~/types/platform";

interface PendingApply {
  entry: LayoutLibraryEntry;
  label: string;
}

interface SaveDraft {
  name: string;
  description: string;
}

type OverwriteAction = "saveAs" | "rename";

type SettingsIssueScope = "global" | "mapper";
type SettingsIssueSeverity = "error" | "warning";

interface SettingsIssue {
  id: string;
  scope: SettingsIssueScope;
  severity: SettingsIssueSeverity;
  title: string;
  description: string;
}

let singleton: any = null;

export function resetSettingsScreenStateForTests() {
  singleton = null;
}

export function useSettingsScreen() {
  if (singleton) return singleton;

  const {
    config,
    settingsDir,
    flush,
    applyPreset,
    markLayoutSavedAs,
    replaceCurrentLayoutSnapshot,
    resetCurrentLayout,
    currentLayoutId,
    isLayoutDirty,
  } = useConfig();
  const library = useLayoutLibrary();
  const mapper = useMapper();
  const platform = usePlatformInfo();
  const theme = useAppTheme();
  const appLocale = useAppLocale();
  const toast = useToast();
  const { t } = useI18n();

  const appearanceItems = computed(() => [
    { label: t("settings.appearanceItems.system"), value: "system" as const },
    { label: t("settings.appearanceItems.light"), value: "light" as const },
    { label: t("settings.appearanceItems.dark"), value: "dark" as const },
  ]);

  const localeItems = computed(() => {
    const resolvedName = localeDisplayName(appLocale.systemLocale.value);
    return [
      {
        label: t("settings.languageAutoResolved", { resolved: resolvedName }),
        value: "auto" as const,
      },
      ...appLocale.available.map((loc) => ({
        label: localeDisplayName(loc),
        value: loc,
      })),
    ];
  });

  const applying = ref("");
  const applyError = ref<string | null>(null);
  const pendingApply = ref<PendingApply | null>(null);

  const saveModalOpen = ref(false);
  const saveName = ref("");
  const saveBusy = ref(false);
  const saveError = ref<string | null>(null);

  const editModalOpen = ref(false);
  const editName = ref("");
  const editDescription = ref("");
  const editBusy = ref(false);
  const editError = ref<string | null>(null);
  const editPending = ref<LayoutLibraryEntry | null>(null);

  const overwriteConfirmOpen = ref(false);
  const overwriteAction = ref<OverwriteAction | null>(null);
  const overwriteTargetName = ref("");

  const resetConfirmOpen = ref(false);
  const resetBusy = ref(false);

  const deletePending = ref<LayoutLibraryEntry | null>(null);
  const deleteBusy = ref(false);

  const deviceOptions = computed(() =>
    mapper.devices.value.map((device) => ({
      label: `${device.name}  —  ${device.path}`,
      value: device.path,
    })),
  );

  const selectedDevice = computed<string>({
    get: () => config.value.settings.inputDevicePath ?? "",
    set: (value: string) => {
      config.value.settings.inputDevicePath = value;
    },
  });

  const mouseOptions = computed(() =>
    mapper.mice.value.map((device) => ({
      label: `${device.name}  —  ${device.path}`,
      value: device.path,
    })),
  );

  const selectedMouse = computed<string>({
    get: () => config.value.settings.inputMouseDevicePath ?? "",
    set: (value: string) => {
      config.value.settings.inputMouseDevicePath = value;
    },
  });

  const currentLayoutDescription = computed(() => config.value.layoutDescription ?? "");

  function describeCapabilityIssue(
    kind: "keyInterception" | "literalInjection" | "layoutDetection" | "systemActions",
    status: CapabilityStatus,
  ): SettingsIssue | null {
    const desktop = platform.info.value?.linux?.desktop ?? platform.info.value?.os ?? "current platform";
    const detail = status.detail?.trim();

    if (kind === "keyInterception" && !status.available) {
      return {
        id: "mapper-key-interception",
        scope: "mapper",
        severity: "error",
        title: t("settings.issues.mapperStartTitle"),
        description: detail || t("settings.issues.mapperStartBody"),
      };
    }

    if (kind === "literalInjection" && !status.available) {
      return {
        id: "mapper-literal-injection",
        scope: "mapper",
        severity: "warning",
        title: t("settings.issues.literalInjectionTitle"),
        description:
          detail || t("settings.issues.literalInjectionBody"),
      };
    }

    if (kind === "layoutDetection" && !status.supported) {
      return {
        id: "global-layout-detection-unsupported",
        scope: "global",
        severity: "warning",
        title: t("settings.issues.layoutDetectionUnsupportedTitle"),
        description: t("settings.issues.layoutDetectionUnsupportedBody", { desktop }),
      };
    }

    if (kind === "layoutDetection" && !status.available) {
      return {
        id: "global-layout-detection-unavailable",
        scope: "global",
        severity: "warning",
        title: t("settings.issues.layoutDetectionUnavailableTitle"),
        description:
          detail || t("settings.issues.layoutDetectionUnavailableBody", { desktop }),
      };
    }

    if (kind === "systemActions" && !status.supported) {
      return {
        id: "global-system-actions-unsupported",
        scope: "global",
        severity: "warning",
        title: t("settings.issues.systemActionsUnsupportedTitle"),
        description: t("settings.issues.systemActionsUnsupportedBody", { desktop }),
      };
    }

    if (kind === "systemActions" && !status.available) {
      return {
        id: "global-system-actions-unavailable",
        scope: "global",
        severity: "warning",
        title: t("settings.issues.systemActionsUnavailableTitle"),
        description:
          detail || t("settings.issues.systemActionsUnavailableBody", { desktop }),
      };
    }

    return null;
  }

  const settingsIssues = computed<SettingsIssue[]>(() => {
    if (platform.error.value) {
      return [
        {
          id: "platform-check-error",
          scope: "global",
          severity: "warning",
          title: t("settings.issues.platformCheckTitle"),
          description: platform.error.value,
        },
      ];
    }

    const capabilities = platform.info.value?.capabilities;
    if (!capabilities) return [];

    return [
      describeCapabilityIssue("keyInterception", capabilities.key_interception),
      describeCapabilityIssue("literalInjection", capabilities.literal_injection),
      describeCapabilityIssue("layoutDetection", capabilities.layout_detection),
      describeCapabilityIssue("systemActions", capabilities.system_actions),
    ].filter((issue): issue is SettingsIssue => issue !== null);
  });

  const globalIssues = computed(() =>
    settingsIssues.value.filter((issue) => issue.scope === "global"),
  );

  const mapperIssues = computed(() =>
    settingsIssues.value.filter((issue) => issue.scope === "mapper"),
  );

  const hasErrorIssues = computed(() =>
    settingsIssues.value.some((issue) => issue.severity === "error"),
  );

  const settingsBanner = computed(() => {
    if (!settingsIssues.value.length) return null;

    return {
      color: hasErrorIssues.value ? "error" : "warning",
      icon: hasErrorIssues.value ? "i-lucide-circle-alert" : "i-lucide-triangle-alert",
      title: hasErrorIssues.value
        ? t("settings.issues.bannerErrorTitle")
        : t("settings.issues.bannerWarningTitle"),
      issues: settingsIssues.value,
    } as const;
  });

  function nextAvailableName(baseName: string): string {
    let candidate = baseName;
    let index = 2;
    while (library.layoutExists(candidate)) {
      candidate = `${baseName} ${index}`;
      index += 1;
    }
    return candidate;
  }

  function validateNameOrSetError(name: string, target: "save" | "edit"): string | null {
    const normalized = normalizeLayoutName(name);
    const code = validateLayoutName(normalized);
    if (!code) return normalized;

    const key =
      code === "empty"
        ? "settings.saveErrorEmpty"
        : "settings.saveErrorInvalidName";
    const message = t(key);
    if (target === "save") saveError.value = message;
    else editError.value = message;
    return null;
  }

  function requestApply(entry: LayoutLibraryEntry) {
    applyError.value = null;
    pendingApply.value = {
      entry,
      label: entry.name,
    };
  }

  function cancelApply() {
    pendingApply.value = null;
  }

  async function confirmApply() {
    const target = pendingApply.value;
    if (!target) return;
    applying.value = target.entry.id;
    try {
      const preset = await library.loadPreset(target.entry.id);
      if (!preset) {
        applyError.value = t("settings.loadFailed", { name: target.entry.name });
        return;
      }
      await applyPreset(preset, target.entry.id);
      toast.add({
        title: t('settings.layoutApplied', { name: target.entry.name }),
        color: 'success',
        icon: 'i-lucide-check',
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

  async function performSave(overwrite = true) {
    const name = validateNameOrSetError(saveName.value, "save");
    if (!name) return;

    if (
      !overwrite &&
      library.layoutExists(name) &&
      (!isUserLayoutId(currentLayoutId.value) ||
        userLayoutNameFromId(currentLayoutId.value!) !== name)
    ) {
      overwriteAction.value = "saveAs";
      overwriteTargetName.value = name;
      overwriteConfirmOpen.value = true;
      return;
    }

    saveBusy.value = true;
    saveError.value = null;
    try {
      config.value.layoutDescription = config.value.layoutDescription?.trim() || undefined;
      const preset = extractPresetFromConfig(config.value);
      const savedName = await library.saveUserPreset(name, preset, overwrite);
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

  function openEditModal(entry: LayoutLibraryEntry) {
    editPending.value = entry;
    editName.value = entry.name;
    editDescription.value =
      currentLayoutId.value === entry.id
        ? currentLayoutDescription.value
        : entry.description ?? "";
    editError.value = null;
    editModalOpen.value = true;
  }

  async function performEdit(overwrite = false) {
    const entry = editPending.value;
    if (!entry) return;
    const oldName = entry.name;
    const newName = validateNameOrSetError(editName.value, "edit");
    if (!newName) return;

    const collides =
      newName !== oldName &&
      library.layoutExists(newName);
    if (collides && !overwrite) {
      overwriteAction.value = "rename";
      overwriteTargetName.value = newName;
      overwriteConfirmOpen.value = true;
      return;
    }

    editBusy.value = true;
    editError.value = null;
    try {
      const previousDescription = config.value.layoutDescription;
      const isCurrent = currentLayoutId.value === entry.id;
      const loadedPreset = isCurrent ? null : await library.loadPreset(entry.id);
      const preset = isCurrent
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
        overwrite,
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
        if (config.value.settings.autoDefaultLayoutId === oldId) {
          config.value.settings.autoDefaultLayoutId = newId;
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
  }

  function requestReset() {
    resetConfirmOpen.value = true;
  }

  async function confirmReset() {
    resetBusy.value = true;
    try {
      await resetCurrentLayout();
      resetConfirmOpen.value = false;
    } catch (error) {
      applyError.value = error instanceof Error ? error.message : String(error);
    } finally {
      resetBusy.value = false;
    }
  }

  function closeResetConfirm() {
    resetConfirmOpen.value = false;
  }

  async function confirmOverwrite() {
    const action = overwriteAction.value;
    overwriteConfirmOpen.value = false;
    overwriteAction.value = null;
    if (action === "saveAs") {
      await performSave(true);
      return;
    }
    if (action === "rename") {
      await performEdit(true);
    }
  }

  function closeOverwriteConfirm() {
    overwriteConfirmOpen.value = false;
    overwriteAction.value = null;
  }

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
      applyError.value = error instanceof Error ? error.message : String(error);
    } finally {
      deleteBusy.value = false;
    }
  }

  function clearDeletePending() {
    deletePending.value = null;
  }

  async function toggleMapper() {
    try {
      await flush();
      if (mapper.status.value.running) {
        await mapper.stop();
        return;
      }
      if (!selectedDevice.value) return;
      await mapper.start(
        selectedDevice.value,
        selectedMouse.value || undefined,
      );
    } catch (error) {
      mapper.error.value =
        error instanceof Error ? error.message : String(error);
    }
  }

  onMounted(async () => {
    await Promise.all([
      mapper.refreshDevices(),
      mapper.refreshStatus(),
      platform.refresh(),
      library.refresh(),
    ]);
  });

  singleton = {
    config,
    settingsDir,
    currentLayoutId,
    currentLayoutDescription,
    isLayoutDirty,
    library,
    mapper,
    platform,
    settingsBanner,
    globalIssues,
    mapperIssues,
    theme,
    appLocale,
    appearanceItems,
    localeItems,
    applying,
    applyError,
    pendingApply,
    requestApplyEntry: requestApply,
    cancelApply,
    confirmApply,
    createFromEmpty,
    createFromIvanK,
    saveModalOpen,
    saveName,
    saveBusy,
    saveError,
    saveCurrentLayout,
    openSaveModal,
    openSaveAsModal,
    performSave,
    closeSaveModal,
    editModalOpen,
    editName,
    editDescription,
    editBusy,
    editError,
    editPending,
    openEditModal,
    performEdit,
    closeEditModal,
    overwriteConfirmOpen,
    overwriteTargetName,
    confirmOverwrite,
    closeOverwriteConfirm,
    resetConfirmOpen,
    resetBusy,
    requestReset,
    confirmReset,
    closeResetConfirm,
    deletePending,
    deleteBusy,
    confirmDelete,
    clearDeletePending,
    deviceOptions,
    selectedDevice,
    mouseOptions,
    selectedMouse,
    toggleMapper,
  };
  return singleton;
}
