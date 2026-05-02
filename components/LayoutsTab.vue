<script setup lang="ts">
import { computed } from "vue";
import HomeHelpCard from "~/components/HomeHelpCard.vue";
import HomePlatformStatusCard from "~/components/HomePlatformStatusCard.vue";
import LayoutsLibraryCard from "~/components/features/settings/LayoutsLibraryCard.vue";
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import { isLayoutInAuto, orderLayoutIds } from "~/utils/layoutAutoSwitch";
import { LAYOUT_MODE_MANUAL, LAYOUT_MODE_AUTO } from "~/types/config";

const {
  config,
  currentLayoutId,
  currentLayoutDescription,
  isLayoutDirty,
  library,
  applying,
  applyError,
  pendingApply: _pendingApply,
  requestApplyEntry,
  cancelApply: _cancelApply,
  confirmApply: _confirmApply,
  createFromEmpty,
  createFromIvanK,
  saveModalOpen: _saveModalOpen,
  saveName: _saveName,
  saveBusy: _saveBusy,
  saveError: _saveError,
  saveCurrentLayout,
  openSaveModal: _openSaveModal,
  openSaveAsModal,
  performSave: _performSave,
  closeSaveModal: _closeSaveModal,
  editModalOpen: _editModalOpen,
  editName: _editName,
  editDescription: _editDescription,
  editBusy: _editBusy,
  editError: _editError,
  editPending: _editPending,
  openEditModal: _openEditModal,
  performEdit: _performEdit,
  closeEditModal: _closeEditModal,
  overwriteConfirmOpen: _overwriteConfirmOpen,
  overwriteTargetName: _overwriteTargetName,
  confirmOverwrite: _confirmOverwrite,
  closeOverwriteConfirm: _closeOverwriteConfirm,
  resetConfirmOpen: _resetConfirmOpen,
  resetBusy: _resetBusy,
  requestReset: _requestReset,
  confirmReset: _confirmReset,
  closeResetConfirm: _closeResetConfirm,
  deletePending,
  deleteBusy: _deleteBusy,
  confirmDelete: _confirmDelete,
  clearDeletePending: _clearDeletePending,
  globalBanner,
} = useSettingsScreen();

const { activeAutoLayoutId } = useLayoutSwitcher()
const manualActiveLayoutId = computed(() => config.value.settings.manualActiveLayoutId)

const { t } = useI18n();

const layoutMode = computed({
  get: () => config.value.settings.layoutMode,
  set: (value: typeof LAYOUT_MODE_MANUAL | typeof LAYOUT_MODE_AUTO) => {
    config.value.settings.layoutMode = value;
  },
});

const modeOptions = computed(() => [
  { label: t("home.modeManual"), value: LAYOUT_MODE_MANUAL },
  { label: t("home.modeAuto"), value: LAYOUT_MODE_AUTO },
]);

const activeMapperLayoutLabel = computed(() => {
  const activeId = layoutMode.value === LAYOUT_MODE_AUTO
    ? activeAutoLayoutId.value
    : manualActiveLayoutId.value;
  if (!activeId) return t("home.activeLayoutNative");
  const entry = library.entries.value.find((e: LayoutLibraryEntry) => e.id === activeId);
  return entry?.name ?? activeId;
});

const orderedEntries = computed<LayoutLibraryEntry[]>(() => {
  const entries: LayoutLibraryEntry[] = library.entries.value;
  if (config.value.settings.layoutMode !== LAYOUT_MODE_AUTO) return entries;
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
  const defaultId = config.value.settings.autoDefaultLayoutId;
  if (defaultId) {
    const defaultRule = map[defaultId];
    if (!defaultRule || !defaultRule.disabledInAuto) {
      set.add(defaultId);
    }
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

const layoutIds = computed(() => orderedEntries.value.map((e) => e.id));
const { selectedId, select, containerRef } = useListKeyboardNavigation({
  ids: layoutIds,
  move: (id: string, delta: number) => {
    const entry = orderedEntries.value.find((e) => e.id === id);
    if (!entry) return;
    moveLayout(entry, delta < 0 ? "up" : "down");
  },
});
</script>

<template>
  <div class="w-full space-y-4">
    <div class="space-y-1">
      <h2 class="text-lg font-semibold">{{ $t("home.title") }}</h2>
      <p class="text-sm text-(--ui-text-muted)">
        {{ $t("home.subtitle") }}
      </p>
    </div>

    <UAlert
      v-if="globalBanner"
      :color="globalBanner.color"
      variant="soft"
      :icon="globalBanner.icon"
      :title="globalBanner.title"
    >
      <template #description>
        <ul class="space-y-2 text-sm">
          <li
            v-for="issue in globalBanner.issues"
            :key="issue.id"
          >
            <span class="font-medium">{{ issue.title }}</span>
            <span class="text-(--ui-text-muted)"> — {{ issue.description }}</span>
          </li>
        </ul>
      </template>
    </UAlert>

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

    <UCard v-if="activeMapperLayoutLabel" variant="subtle">
      <div class="flex items-center gap-2 text-sm">
        <UIcon name="i-lucide-layout-template" class="text-(--ui-text-muted)" />
        <span class="text-(--ui-text-muted)">{{ $t("home.activeLayoutLabel") }}:</span>
        <span class="font-semibold">{{ activeMapperLayoutLabel }}</span>
      </div>
    </UCard>

    <UCard
      v-if="layoutMode === 'manual' && !manualActiveLayoutId"
      color="warning"
      variant="subtle"
    >
      <div class="flex items-start gap-2 text-sm">
        <UIcon name="i-lucide-triangle-alert" class="shrink-0 mt-0.5" />
        <div>
          <p class="font-semibold">{{ $t("home.manualNoActiveTitle") }}</p>
          <p class="text-(--ui-text-muted)">{{ $t("home.manualNoActiveBody") }}</p>
        </div>
      </div>
    </UCard>

    <div class="space-y-4">
      <div ref="containerRef">
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
          :selected-id="selectedId"
          @select="select"
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
      </div>

      <HomeHelpCard />
      <HomePlatformStatusCard />
    </div>
  </div>
</template>
