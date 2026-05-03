import type { AppConfig, AppearancePreference, LocalePreference } from "~/types/config";
import type { CapabilityStatus } from "~/types/platform";
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import type { LayoutEditMode } from "./useLayoutEdit";

export interface PendingApply {
  entry: LayoutLibraryEntry;
  label: string;
}

export type OverwriteAction = "saveAs" | "rename";

export type SettingsIssueScope = "global" | "mapper";
export type SettingsIssueSeverity = "error" | "warning";

export interface SettingsIssue {
  id: string;
  scope: SettingsIssueScope;
  severity: SettingsIssueSeverity;
  title: string;
  description: string;
}

export interface SettingsScreenState {
  config: Ref<AppConfig>;
  settingsDir: Ref<string>;
  currentLayoutId: ComputedRef<string | undefined>;
  currentLayoutDescription: ComputedRef<string>;
  isLayoutDirty: ComputedRef<boolean>;
  library: ReturnType<typeof useLayoutLibrary>;
  mapper: ReturnType<typeof useMapper>;
  platform: ReturnType<typeof usePlatformInfo>;
  theme: ReturnType<typeof useAppTheme>;
  appLocale: ReturnType<typeof useAppLocale>;
  globalBanner: ComputedRef<{ color: string; icon: string; title: string; issues: SettingsIssue[] } | null>;
  globalIssues: ComputedRef<SettingsIssue[]>;
  mapperIssues: ComputedRef<SettingsIssue[]>;
  appearanceItems: ComputedRef<{ label: string; value: AppearancePreference }[]>;
  localeItems: ComputedRef<{ label: string; value: LocalePreference }[]>;
  applying: Ref<string>;
  applyError: Ref<string | null>;
  pendingApply: Ref<PendingApply | null>;
  requestApplyEntry: (entry: LayoutLibraryEntry) => void;
  cancelApply: () => void;
  confirmApply: () => Promise<void>;
  createFromEmpty: () => Promise<void>;
  createFromIvanK: () => Promise<void>;
  saveModalOpen: Ref<boolean>;
  saveName: Ref<string>;
  saveBusy: Ref<boolean>;
  saveError: Ref<string | null>;
  saveCurrentLayout: () => Promise<void>;
  openSaveModal: () => void;
  openSaveAsModal: () => void;
  performSave: (overwrite?: boolean) => Promise<void>;
  closeSaveModal: () => void;
  editModalOpen: Ref<boolean>;
  editName: Ref<string>;
  editDescription: Ref<string>;
  editMode: Ref<LayoutEditMode>;
  editBusy: Ref<boolean>;
  editError: Ref<string | null>;
  editPending: Ref<LayoutLibraryEntry | null>;
  openEditModal: (entry: LayoutLibraryEntry, mode?: LayoutEditMode) => void;
  performEdit: (overwrite?: boolean) => Promise<void>;
  closeEditModal: () => void;
  overwriteConfirmOpen: Ref<boolean>;
  overwriteTargetName: Ref<string>;
  confirmOverwrite: () => Promise<void>;
  closeOverwriteConfirm: () => void;
  resetConfirmOpen: Ref<boolean>;
  resetBusy: Ref<boolean>;
  requestReset: () => void;
  confirmReset: () => Promise<void>;
  closeResetConfirm: () => void;
  deletePending: Ref<LayoutLibraryEntry | null>;
  deleteBusy: Ref<boolean>;
  confirmDelete: () => Promise<void>;
  clearDeletePending: () => void;
  deviceOptions: ComputedRef<{ label: string; value: string }[]>;
  selectedDevice: ComputedRef<string>;
  mouseOptions: ComputedRef<{ label: string; value: string }[]>;
  selectedMouse: ComputedRef<string>;
  toggleMapper: () => Promise<void>;
}

export type CapabilityKind =
  | "keyInterception"
  | "literalInjection"
  | "layoutDetection"
  | "systemActions";

export type CapabilityIssueFactory = (
  kind: CapabilityKind,
  status: CapabilityStatus,
) => SettingsIssue | null;
