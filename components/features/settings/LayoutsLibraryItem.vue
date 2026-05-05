<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import type { LayoutMode } from "~/types/config";
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
  applying: string;
  activeAutoLayoutId?: string;
  manualActiveLayoutId?: string;
  autoIncludedIds: Set<string>;
}>();

const emit = defineEmits<{
  requestEdit: [entry: LayoutLibraryEntry];
  updateDescription: [entry: LayoutLibraryEntry, description: string];
  requestApplyEntry: [entry: LayoutLibraryEntry];
  requestDelete: [entry: LayoutLibraryEntry];
  moveUp: [entry: LayoutLibraryEntry];
  moveDown: [entry: LayoutLibraryEntry];
  activateManual: [entryId: string];
  toggleAuto: [entryId: string, value: boolean];
  openWhitelist: [entryId: string];
  openBlacklist: [entryId: string];
}>();

const { config } = useConfig();
const { t } = useI18n();

const isEditingDescription = ref(false);
const editValue = ref("");
const textareaRef = ref<import('vue').ComponentPublicInstance | null>(null);

function startEdit() {
  editValue.value = description.value || "";
  isEditingDescription.value = true;
  nextTick(() => {
    textareaRef.value?.$el?.querySelector('textarea')?.focus();
  });
}

function cancelEdit() {
  isEditingDescription.value = false;
}

async function saveEdit() {
  if (editValue.value !== description.value) {
    emit("updateDescription", props.entry, editValue.value);
  }
  isEditingDescription.value = false;
}

function entryIsIncluded(entryId: string) {
  return props.autoIncludedIds.has(entryId);
}

function entryToggleAuto(entryId: string, value: boolean) {
  emit("toggleAuto", entryId, value);
}

function entryIsEnabledInAuto(entryId: string) {
  const rule = config.value.settings.layoutConditions[entryId];
  return rule?.enabledInAuto === true;
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
  emit("openWhitelist", entryId);
}

function openBlacklist(entryId: string) {
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
    class="relative p-4 rounded-xl border flex gap-6 group transition-all duration-150 hover:shadow-lg"
    :class="[
      layoutMode === 'auto' && !entryIsIncluded(entry.id) ? 'opacity-50 grayscale-[30%]' : '',
      'border-(--ui-border) bg-(--ui-bg-muted)/40 hover:bg-(--ui-bg-muted)/60 hover:border-(--ui-primary)/50 hover:shadow-(--ui-primary)/5'
    ]"
  >
    <div class="flex-1 flex flex-col gap-2">
      <div class="flex items-center gap-2 min-w-0">
        <div class="min-w-0 flex-1">
          <div class="font-medium truncate flex items-center gap-2 flex-wrap">
            <span class="truncate">{{ entry.name }}</span>
            <AppTooltip :text="$t('settings.renameLayoutAria', { name: entry.name })">
              <UButton
                icon="i-lucide-pencil"
                variant="ghost"
                color="neutral"
                size="xs"
                square
                class="opacity-0 group-hover:opacity-100 focus:opacity-100 transition-opacity duration-150"
                :aria-label="$t('settings.renameLayoutAria', { name: entry.name })"
                @click="$emit('requestEdit', entry)"
              />
            </AppTooltip>
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
          <div v-if="isEditingDescription" class="mt-1 flex flex-col gap-1.5">
            <UTextarea
              ref="textareaRef"
              v-model="editValue"
              :placeholder="$t('rules.addDescription')"
              autoresize
              :rows="1"
              size="sm"
              class="w-full"
              @keydown.esc="cancelEdit"
              @keydown.enter.ctrl="saveEdit"
            />
            <div class="flex items-center gap-1.5">
              <UButton
                size="xs"
                color="primary"
                @click="saveEdit"
              >
                {{ $t('common.save') }}
              </UButton>
              <UButton
                size="xs"
                color="neutral"
                variant="ghost"
                @click="cancelEdit"
              >
                {{ $t('common.cancel') }}
              </UButton>
              <span class="text-[10px] text-(--ui-text-muted) ml-auto">
                Ctrl + Enter to save
              </span>
            </div>
          </div>
          <template v-else>
            <button
              v-if="description"
              type="button"
              class="mt-0.5 line-clamp-2 w-full cursor-text rounded-sm text-left text-sm text-(--ui-text-muted) hover:text-(--ui-text) focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-(--ui-primary) focus-visible:ring-offset-2 focus-visible:ring-offset-(--ui-bg)"
              @click="startEdit"
            >
              {{ description }}
            </button>
            <UButton
              v-else
              color="neutral"
              variant="link"
              size="xs"
              class="mt-0.5 px-0"
              @click="startEdit"
            >
              {{ $t('rules.addDescription') }}
            </UButton>
          </template>
          <div v-if="layoutMode === 'auto'" class="flex items-center justify-between gap-2 mt-1" @click.stop>
            <div class="flex items-center gap-2 flex-wrap">
              <AppTooltip :text="$t('rules.blacklistHint')">
                <UButton
                  size="xs"
                  color="neutral"
                  variant="outline"
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
          </div>
        </div>
      </div>
    </div>

    <div class="w-px bg-(--ui-border) self-stretch"></div>

    <div class="min-w-[12rem] max-w-[16rem] flex flex-col gap-2">
      <div
        class="flex items-center"
        :class="layoutMode === 'auto' ? 'justify-between' : 'justify-end'"
      >
        <div
          v-if="layoutMode === 'auto'"
          class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150"
        >
          <AppTooltip :text="$t('common.moveUp')">
            <UButton
              icon="i-lucide-arrow-up"
              variant="ghost"
              color="neutral"
              size="sm"
              square
              :aria-label="$t('settings.moveLayoutUpAria', { name: entry.name })"
              :disabled="isFirst"
              @click="$emit('moveUp', entry)"
            />
          </AppTooltip>
          <AppTooltip :text="$t('common.moveDown')">
            <UButton
              icon="i-lucide-arrow-down"
              variant="ghost"
              color="neutral"
              size="sm"
              square
              :aria-label="$t('settings.moveLayoutDownAria', { name: entry.name })"
              :disabled="isLast"
              @click="$emit('moveDown', entry)"
            />
          </AppTooltip>
        </div>

        <div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
          <AppTooltip :text="$t('settings.deleteAria')">
            <UButton
              icon="i-lucide-trash-2"
              variant="ghost"
              color="neutral"
              size="sm"
              square
              :aria-label="$t('settings.deleteAria')"
              @click="$emit('requestDelete', entry)"
            />
          </AppTooltip>
        </div>
      </div>
      <div class="flex items-center justify-end">
        <UButton
          size="sm"
          color="primary"
          variant="outline"
          icon="i-lucide-folder-open"
          :loading="applying === entry.id"
          :disabled="!!applying || currentLayoutId === entry.id"
          @click="$emit('requestApplyEntry', entry)"
        >
          {{ $t('settings.openLayoutBtn') }}
        </UButton>
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
        <AppTooltip :text="$t('rules.autoIncludeHint')">
          <div class="flex items-center gap-1.5 cursor-pointer">
            <USwitch
              :model-value="entryIsEnabledInAuto(entry.id)"
              @update:model-value="entryToggleAuto(entry.id, $event === true)"
            />
            <span
              class="text-xs text-(--ui-text-muted) select-none"
              @click.stop="entryToggleAuto(entry.id, !entryIsEnabledInAuto(entry.id))"
            >{{ $t('rules.autoIncludeLabel') }}</span>
          </div>
        </AppTooltip>
      </div>
    </div>
  </li>
</template>
