<script setup lang="ts">
import { computed } from "vue";
import HomeInfoCard from "~/components/HomeInfoCard.vue";
import LayoutsLibraryCard from "~/components/features/settings/LayoutsLibraryCard.vue";
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import { isLayoutInAuto, orderLayoutIds } from "~/utils/layoutAutoSwitch";

const {
  config,
  currentLayoutId,
  currentLayoutDescription,
  isLayoutDirty,
  library,
  applying,
  applyError,
  pendingApply,
  requestApplyEntry,
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
} = useSettingsScreen();

const { activeAutoLayoutId } = useLayoutSwitcher()
const manualActiveLayoutId = computed(() => config.value.settings.manualActiveLayoutId)

const { t } = useI18n();

const layoutMode = computed({
  get: () => config.value.settings.layoutMode,
  set: (value: "manual" | "auto") => {
    config.value.settings.layoutMode = value;
  },
});

const modeOptions = computed(() => [
  { label: t("home.modeManual"), value: "manual" as const },
  { label: t("home.modeAuto"), value: "auto" as const },
]);

const orderedEntries = computed<LayoutLibraryEntry[]>(() => {
  const entries: LayoutLibraryEntry[] = library.entries.value;
  if (config.value.settings.layoutMode !== "auto") return entries;
  const order = orderLayoutIds(
    entries.map((e: LayoutLibraryEntry) => e.id),
    config.value.settings.layoutOrder,
  );
  const byId = new Map(
    entries.map((e: LayoutLibraryEntry) => [e.id, e] as const),
  );
  return order
    .map((id) => byId.get(id))
    .filter((e): e is LayoutLibraryEntry => !!e);
});

const autoIncludedIds = computed(() => {
  const set = new Set<string>();
  const map = config.value.settings.layoutConditions;
  for (const id of Object.keys(map)) {
    if (isLayoutInAuto(map[id])) set.add(id);
  }
  return set;
});

const autoDefaultLayoutId = computed(
  () => config.value.settings.autoDefaultLayoutId,
);

function syncLayoutOrder(ids: string[]) {
  config.value.settings.layoutOrder = ids;
}

function moveLayout(entry: LayoutLibraryEntry, direction: "up" | "down") {
  const current = orderedEntries.value.map((e) => e.id);
  const index = current.indexOf(entry.id);
  const target = direction === "up" ? index - 1 : index + 1;
  if (index < 0 || target < 0 || target >= current.length) return;
  [current[index], current[target]] = [current[target]!, current[index]!];
  syncLayoutOrder(current);
}
</script>

<template>
  <div class="mx-auto w-full max-w-5xl space-y-4">
    <div class="space-y-1">
      <h2 class="text-lg font-semibold">{{ $t("home.title") }}</h2>
      <p class="text-sm text-(--ui-text-muted)">
        {{ $t("home.subtitle") }}
      </p>
    </div>

    <UCard>
      <div class="flex items-center justify-between gap-3 flex-wrap">
        <div class="min-w-0">
          <h3 class="text-sm font-semibold">{{ $t("home.modeLabel") }}</h3>
          <p class="text-xs text-(--ui-text-muted) mt-0.5">
            {{
              layoutMode === "auto"
                ? $t("home.modeAutoHint")
                : $t("home.modeManualHint")
            }}
          </p>
        </div>
        <URadioGroup
          v-model="layoutMode"
          :items="modeOptions"
          orientation="horizontal"
          value-key="value"
        />
      </div>
    </UCard>

    <div class="space-y-4">
      <LayoutsLibraryCard
        :entries="orderedEntries"
        :current-layout-id="currentLayoutId"
        :current-layout-description="currentLayoutDescription"
        :is-layout-dirty="isLayoutDirty"
        :applying="applying"
        :apply-error="applyError"
        :library-error="library.error.value"
        :layouts-dir="library.layoutsDir.value"
        :layout-mode="layoutMode"
        :auto-included-ids="autoIncludedIds"
        :auto-default-layout-id="autoDefaultLayoutId"
        :active-auto-layout-id="activeAutoLayoutId"
        :manual-active-layout-id="manualActiveLayoutId"
        @save-current="saveCurrentLayout"
        @save-as="openSaveAsModal"
        @request-apply-entry="requestApplyEntry"
        @create-from-empty="createFromEmpty"
        @create-from-ivan-k="createFromIvanK"
        @request-edit="openEditModal"
        @request-reset="requestReset"
        @request-delete="(entry) => (deletePending = entry)"
        @move-up="(entry) => moveLayout(entry, 'up')"
        @move-down="(entry) => moveLayout(entry, 'down')"
      />

      <HomeInfoCard />
    </div>
  </div>
</template>
