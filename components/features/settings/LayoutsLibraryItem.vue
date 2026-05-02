<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import type { LayoutMode } from "~/types/config";
import { useLayoutConditions } from "~/composables/useLayoutConditions";
import type { ConditionKind } from "~/composables/useLayoutConditions";
import type { LayoutConditionSet } from "~/types/config";
import AppTooltip from "~/components/shared/AppTooltip.vue";

const props = defineProps<{
  entry: LayoutLibraryEntry;
  index: number;
  isFirst: boolean;
  isLast: boolean;
  layoutMode: LayoutMode;
  currentLayoutId?: string;
  currentLayoutDescription?: string;
  isLayoutDirty: boolean;
  activeAutoLayoutId?: string;
  manualActiveLayoutId?: string;
  autoDefaultLayoutId?: string;
  autoIncludedIds: Set<string>;
  selectedId?: string | null;
}>();

const emit = defineEmits<{
  select: [id: string];
  requestEdit: [entry: LayoutLibraryEntry];
  requestDelete: [entry: LayoutLibraryEntry];
  moveUp: [entry: LayoutLibraryEntry];
  moveDown: [entry: LayoutLibraryEntry];
  activateManual: [entryId: string];
  toggleDefault: [entryId: string, value: boolean];
  toggleAuto: [entryId: string, value: boolean];
  openWhitelist: [entryId: string];
  openBlacklist: [entryId: string];
}>();

const { config } = useConfig();
const { t } = useI18n();
const { setDisabledInAuto, setAsDefault } = useLayoutConditions();

function entryIsDefault(entryId: string) {
  return props.autoDefaultLayoutId === entryId;
}

function entryIsIncluded(entryId: string) {
  return props.autoIncludedIds.has(entryId);
}

function entryHasWhitelist(entryId: string) {
  return !!config.value.settings.layoutConditions[entryId]?.whitelist;
}

function entryHasConditions(entryId: string) {
  const rule = config.value.settings.layoutConditions[entryId];
  return !!(rule?.whitelist || rule?.blacklist);
}

function entryToggleDefault(entryId: string, value: boolean) {
  setAsDefault(value ? entryId : undefined);
}

function entryToggleAuto(entryId: string, value: boolean) {
  setDisabledInAuto(entryId, !value);
}

function entryIsEnabledInAuto(entryId: string) {
  const rule = config.value.settings.layoutConditions[entryId];
  if (rule?.disabledInAuto) return false;
  if (entryIsDefault(entryId)) return true;
  if (!rule) return false;
  if (!rule.whitelist && !rule.blacklist) return false;
  return true;
}

function entryAutoSwitchDisabledReason(entryId: string): string | undefined {
  if (entryIsDefault(entryId)) return undefined;
  if (!entryHasConditions(entryId)) return t("rules.autoIncludeDisabledHintNoConditions");
  return undefined;
}

function handleEntryClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  if (target?.closest('input, textarea, select, button, [role="dialog"], [role="listbox"]')) return;
  emit("select", props.entry.id);
  emit("requestEdit", props.entry);
}

function entryActivateManual() {
  emit("activateManual", props.entry.id);
}

function summarize(kind: ConditionKind, set: LayoutConditionSet | undefined): string {
  const prefix = kind === "whitelist" ? t("rules.whitelistPrefix") : t("rules.blacklistPrefix");
  if (!set || (!set.gameMode && set.layouts.length === 0 && (!set.apps || set.apps.length === 0))) {
    return prefix + "…";
  }
  const parts: string[] = [];
  if (set.gameMode === "on") parts.push(t("rules.gameModeOnSummary"));
  else if (set.gameMode === "off") parts.push(t("rules.gameModeOffSummary"));
  if (set.layouts.length > 0) parts.push(...set.layouts);
  if (set.apps && set.apps.length > 0) parts.push(...set.apps);
  return prefix + " " + parts.join(", ");
}

function entryWhitelistSummary(entryId: string) {
  return summarize("whitelist", config.value.settings.layoutConditions[entryId]?.whitelist);
}

function entryBlacklistSummary(entryId: string) {
  return summarize("blacklist", config.value.settings.layoutConditions[entryId]?.blacklist);
}

function openWhitelist(entryId: string) {
  if (entryIsDefault(entryId)) return;
  emit("openWhitelist", entryId);
}

function openBlacklist(entryId: string) {
  if (entryIsDefault(entryId)) return;
  emit("openBlacklist", entryId);
}

const description = computed(() => {
  return props.currentLayoutId === props.entry.id
    ? props.currentLayoutDescription
    : props.entry.description;
});
</script>

<template>
  <li
    class="relative p-4 rounded-xl border flex gap-6 group transition-all duration-150 hover:shadow-lg cursor-pointer"
    :class="[
      layoutMode === 'auto' && !entryIsIncluded(entry.id) && !entryIsDefault(entry.id) ? 'opacity-50 grayscale-[30%]' : '',
      selectedId === entry.id
        ? 'border-(--ui-primary) ring-1 ring-(--ui-primary) bg-(--ui-bg-muted)/60 shadow-lg shadow-(--ui-primary)/5'
        : 'border-(--ui-border) bg-(--ui-bg-muted)/40 hover:bg-(--ui-bg-muted)/60 hover:border-(--ui-primary)/50 hover:shadow-(--ui-primary)/5'
    ]"
    @click="handleEntryClick"
  >
    <div class="flex-1 flex flex-col gap-2">
      <div class="flex items-center gap-2 min-w-0">
        <div
          v-if="layoutMode === 'auto'"
          class="flex flex-col gap-0.5 shrink-0"
          @click.stop
        >
          <AppTooltip :text="$t('common.moveUp')">
            <UButton
              color="neutral"
              variant="ghost"
              size="xs"
              :square="true"
              icon="i-lucide-chevron-up"
              :aria-label="$t('settings.moveLayoutUpAria', { name: entry.name })"
              :disabled="isFirst"
              @click="$emit('moveUp', entry)"
            />
          </AppTooltip>
          <AppTooltip :text="$t('common.moveDown')">
            <UButton
              color="neutral"
              variant="ghost"
              size="xs"
              :square="true"
              icon="i-lucide-chevron-down"
              :aria-label="$t('settings.moveLayoutDownAria', { name: entry.name })"
              :disabled="isLast"
              @click="$emit('moveDown', entry)"
            />
          </AppTooltip>
        </div>
        <div class="min-w-0 flex-1">
          <div class="font-medium truncate flex items-center gap-2 flex-wrap">
            {{ entry.name }}
            <UBadge
              v-if="layoutMode === 'auto' && activeAutoLayoutId === entry.id"
              color="success"
              variant="subtle"
              size="sm"
            >
              {{ $t("settings.activeBadge") }} (Auto)
            </UBadge>
            <UBadge
              v-if="layoutMode === 'manual' && manualActiveLayoutId === entry.id"
              color="success"
              variant="subtle"
              size="sm"
            >
              {{ $t("settings.activeBadge") }}
            </UBadge>
            <UBadge
              v-if="currentLayoutId === entry.id"
              color="info"
              variant="subtle"
              size="sm"
            >
              {{ $t('settings.editingBadge') }}
            </UBadge>
            <UBadge
              v-if="currentLayoutId === entry.id && isLayoutDirty"
              color="warning"
              variant="subtle"
              size="sm"
            >
              {{ $t("settings.unsavedBadge") }}
            </UBadge>
          </div>
          <div
            v-if="description"
            class="text-sm text-(--ui-text-muted) line-clamp-2 mt-0.5"
          >
            {{ description }}
          </div>
          <div v-if="layoutMode === 'auto'" class="flex items-center justify-between gap-2 mt-1" @click.stop>
            <div v-if="!entryIsDefault(entry.id)" class="flex items-center gap-2 flex-wrap">
              <AppTooltip :text="$t('rules.blacklistDisabledHint')" :disabled="entryHasWhitelist(entry.id)">
                <UButton
                  size="xs"
                  color="neutral"
                  variant="outline"
                  :disabled="!entryHasWhitelist(entry.id)"
                  @click="openBlacklist(entry.id)"
                >
                  <div class="flex items-center gap-1 min-w-0">
                    <UIcon name="i-lucide-list-x" class="shrink-0" />
                    <span class="truncate">{{ entryBlacklistSummary(entry.id) }}</span>
                  </div>
                </UButton>
              </AppTooltip>
              <UButton
                size="xs"
                color="neutral"
                variant="outline"
                @click="openWhitelist(entry.id)"
              >
                <div class="flex items-center gap-1 min-w-0">
                  <UIcon name="i-lucide-list-checks" class="shrink-0" />
                  <span class="truncate">{{ entryWhitelistSummary(entry.id) }}</span>
                </div>
              </UButton>
            </div>
            <div v-else class="flex-1" />
            <div class="flex items-center gap-1.5 cursor-pointer shrink-0">
              <USwitch
                :model-value="entryIsDefault(entry.id)"
                @update:model-value="entryToggleDefault(entry.id, $event === true)"
              />
              <span class="text-xs text-(--ui-text-muted) select-none" @click.stop="entryToggleDefault(entry.id, !entryIsDefault(entry.id))">{{ $t('rules.autoDefaultLabel') }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="w-px bg-(--ui-border) self-stretch"></div>

    <div class="min-w-[12rem] max-w-[16rem] flex flex-col gap-2">
      <div class="flex items-center justify-end">
        <div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
          <AppTooltip :text="$t('settings.editLayoutAria', { name: entry.name })">
            <UButton
              icon="i-lucide-pencil"
              variant="ghost"
              color="neutral"
              size="sm"
              square
              :aria-label="$t('settings.editLayoutAria', { name: entry.name })"
              @click.stop="$emit('requestEdit', entry)"
            />
          </AppTooltip>
          <AppTooltip :text="$t('settings.deleteAria')">
            <UButton
              icon="i-lucide-trash-2"
              variant="ghost"
              color="neutral"
              size="sm"
              square
              :aria-label="$t('settings.deleteAria')"
              @click.stop="$emit('requestDelete', entry)"
            />
          </AppTooltip>
        </div>
      </div>
      <div v-if="layoutMode === 'manual' && manualActiveLayoutId !== entry.id" class="flex items-center justify-end" @click.stop>
        <AppTooltip :text="$t('rules.activateBtn')">
          <UButton
            size="sm"
            color="primary"
            variant="outline"
            @click.stop="entryActivateManual"
          >
            {{ $t('rules.activateBtn') }}
          </UButton>
        </AppTooltip>
      </div>
      <div v-if="layoutMode === 'auto'" class="flex flex-col gap-2 items-end justify-end" @click.stop>
        <AppTooltip :text="entryAutoSwitchDisabledReason(entry.id)" :disabled="!entryAutoSwitchDisabledReason(entry.id)">
          <div class="flex items-center gap-1.5 cursor-pointer">
            <USwitch
              :model-value="entryIsEnabledInAuto(entry.id)"
              :disabled="!entryHasConditions(entry.id) && !entryIsDefault(entry.id)"
              @update:model-value="entryToggleAuto(entry.id, $event === true)"
            />
            <span
              class="text-xs text-(--ui-text-muted) select-none"
              @click.stop="(!entryHasConditions(entry.id) && !entryIsDefault(entry.id)) ? undefined : entryToggleAuto(entry.id, !entryIsEnabledInAuto(entry.id))"
            >{{ $t('rules.autoIncludeLabel') }}</span>
          </div>
        </AppTooltip>
      </div>
    </div>
  </li>
</template>
