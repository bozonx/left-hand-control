<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import { isUserLayoutId, userLayoutNameFromId } from "~/composables/useLayoutLibrary";
import type { LayoutMode } from "~/types/config";
import { useLayoutConditions } from "~/composables/useLayoutConditions";
import type { ConditionKind } from "~/composables/useLayoutConditions";
import type { LayoutConditionSet } from "~/types/config";
import LayoutConditionsModal from "~/components/features/settings/LayoutConditionsModal.vue";
import AppTooltip from "~/components/shared/AppTooltip.vue";

const props = defineProps<{
    entries: LayoutLibraryEntry[];
    currentLayoutId?: string;
    currentLayoutDescription?: string;
    isLayoutDirty: boolean;
    applying: string;
    applyError?: string | null;
    libraryError?: string | null;
    layoutsDir: string;
    layoutMode: LayoutMode;
    autoIncludedIds: Set<string>;
    autoDefaultLayoutId?: string;
    activeAutoLayoutId?: string;
    manualActiveLayoutId?: string;
    selectedId?: string | null;
}>();

const emit = defineEmits<{
    saveCurrent: [];
    saveAs: [];
    requestApplyEntry: [entry: LayoutLibraryEntry];
    createFromEmpty: [];
    createFromIvanK: [];
    requestEdit: [entry: LayoutLibraryEntry];
    requestReset: [];
    requestDelete: [entry: LayoutLibraryEntry];
    moveUp: [entry: LayoutLibraryEntry];
    moveDown: [entry: LayoutLibraryEntry];
    select: [id: string];
}>();

const { config } = useConfig();
const { t } = useI18n();
const { setDisabledInAuto, setAsDefault } = useLayoutConditions();

const modalOpen = ref(false);
const modalKind = ref<ConditionKind>("whitelist");
const modalLayoutId = ref<string | undefined>(undefined);

const modalLayoutLabel = computed(() => {
    const id = modalLayoutId.value;
    if (!id) return "";
    const entry = props.entries.find((e) => e.id === id);
    if (entry) return entry.name;
    return isUserLayoutId(id) ? userLayoutNameFromId(id) : id;
});

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
    return !!rule?.whitelist;
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
    if (!rule.whitelist) return false;
    return true;
}

function entryAutoSwitchDisabledReason(entryId: string): string | undefined {
    if (entryIsDefault(entryId)) return undefined;
    if (!entryHasWhitelist(entryId)) return t("rules.autoIncludeDisabledHintNoWhitelist");
    return undefined;
}

function handleEntryClick(event: MouseEvent, entry: LayoutLibraryEntry) {
    const target = event.target as HTMLElement | null;
    if (target?.closest('input, textarea, select, button, [role="dialog"], [role="listbox"]')) return;
    emit("select", entry.id);
    if (props.layoutMode === "manual") {
        emit("requestApplyEntry", entry);
    }
}

function entryActivateManual(entryId: string) {
    config.value.settings.manualActiveLayoutId = entryId;
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
    modalLayoutId.value = entryId;
    modalKind.value = "whitelist";
    modalOpen.value = true;
}

function openBlacklist(entryId: string) {
    if (entryIsDefault(entryId)) return;
    modalLayoutId.value = entryId;
    modalKind.value = "blacklist";
    modalOpen.value = true;
}
</script>

<template>
    <UCard>
        <template #header>
            <div class="flex items-center justify-between gap-3 flex-wrap">
                <h2 class="text-sm font-semibold">
                    {{ $t("settings.layoutsTitle") }}
                </h2>
                <div class="flex items-center gap-2">
                    <UButton
                        color="neutral"
                        variant="outline"
                        icon="i-lucide-file-plus"
                        :loading="applying === 'create:empty'"
                        :disabled="!!applying"
                        @click="$emit('createFromEmpty')"
                    >
                        {{ $t("settings.newEmptyLayoutBtn") }}
                    </UButton>
                    <UButton
                        color="neutral"
                        variant="outline"
                        icon="i-lucide-copy-plus"
                        :loading="applying === 'create:ivank'"
                        :disabled="!!applying"
                        @click="$emit('createFromIvanK')"
                    >
                        {{ $t("settings.newFromIvanKBtn") }}
                    </UButton>
                    <UButton
                        color="neutral"
                        variant="outline"
                        icon="i-lucide-copy"
                        :disabled="!currentLayoutId"
                        @click="$emit('saveAs')"
                    >
                        {{ $t("settings.saveAs") }}
                    </UButton>
                </div>
            </div>
        </template>

        <div class="space-y-3">
            <div
                v-if="isLayoutDirty"
                class="flex items-start justify-between gap-3 p-3 rounded border border-(--ui-warning)/40 bg-(--ui-warning)/10 text-sm"
            >
                <div class="flex items-start gap-2 min-w-0">
                    <UIcon
                        name="i-lucide-triangle-alert"
                        class="text-(--ui-warning) mt-0.5 shrink-0"
                    />
                    <div class="min-w-0">
                        <div class="font-semibold">
                            {{ $t("settings.dirtyBadgeTitle") }}
                        </div>
                        <div class="text-(--ui-text-muted)">
                            {{ $t("settings.dirtyBadgeBody") }}
                        </div>
                    </div>
                </div>
                <div class="flex items-center gap-2 shrink-0">
                    <UButton
                        color="neutral"
                        variant="outline"
                        icon="i-lucide-rotate-ccw"
                        @click="$emit('requestReset')"
                    >
                        {{ $t("settings.resetUnsavedBtn") }}
                    </UButton>
                    <UButton
                        color="primary"
                        icon="i-lucide-save"
                        :disabled="!currentLayoutId"
                        @click="$emit('saveCurrent')"
                    >
                        {{ $t("settings.saveCurrent") }}
                    </UButton>
                </div>
            </div>

            <p v-if="applyError" class="text-sm text-(--ui-error)">
                {{ applyError }}
            </p>

            <p v-if="libraryError" class="text-sm text-(--ui-error)">
                {{ libraryError }}
            </p>

            <ul
                class="divide-y divide-(--ui-border) border border-(--ui-border) rounded"
            >
                <li
                    v-for="(entry, index) in entries"
                    :key="entry.id"
                    class="relative p-4 rounded-xl border flex gap-6 group transition-all duration-300 hover:shadow-lg cursor-pointer"
                    :class="[
                        layoutMode === 'auto' && !entryIsIncluded(entry.id) && !entryIsDefault(entry.id) ? 'opacity-50 grayscale-[30%]' : '',
                        selectedId === entry.id
                            ? 'border-(--ui-primary) ring-1 ring-(--ui-primary) bg-(--ui-bg-muted)/60 shadow-lg shadow-(--ui-primary)/5'
                            : 'border-(--ui-border) bg-(--ui-bg-muted)/40 hover:bg-(--ui-bg-muted)/60 hover:border-sky-500/50 hover:shadow-sky-500/5'
                    ]"
                    @click="handleEntryClick($event, entry)"
                >
                    <div class="flex-1 flex flex-col gap-2">
                        <div class="flex items-center gap-2 min-w-0">
                            <div
                                v-if="layoutMode === 'auto'"
                                class="flex flex-col gap-0.5 shrink-0"
                                @click.stop
                            >
                                <UButton
                                    color="neutral"
                                    variant="ghost"
                                    size="xs"
                                    :square="true"
                                    icon="i-lucide-chevron-up"
                                    :aria-label="$t('settings.moveLayoutUpAria', { name: entry.name })"
                                    :disabled="index === 0"
                                    @click="$emit('moveUp', entry)"
                                />
                                <UButton
                                    color="neutral"
                                    variant="ghost"
                                    size="xs"
                                    :square="true"
                                    icon="i-lucide-chevron-down"
                                    :aria-label="$t('settings.moveLayoutDownAria', { name: entry.name })"
                                    :disabled="index === entries.length - 1"
                                    @click="$emit('moveDown', entry)"
                                />
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
                                        Editing
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
                                    v-if="(currentLayoutId === entry.id ? currentLayoutDescription : entry.description) || false"
                                    class="text-sm text-(--ui-text-muted) line-clamp-2 mt-0.5"
                                >
                                    {{ currentLayoutId === entry.id ? currentLayoutDescription : entry.description }}
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

                    <div class="w-52 flex flex-col gap-2">
                        <div class="flex items-center justify-end">
                            <div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-300">
                                <UButton
                                    icon="i-lucide-pencil"
                                    variant="ghost"
                                    color="neutral"
                                    size="sm"
                                    square
                                    :aria-label="$t('settings.editLayoutAria', { name: entry.name })"
                                    @click.stop="$emit('requestEdit', entry)"
                                />
                                <UButton
                                    icon="i-lucide-trash-2"
                                    variant="ghost"
                                    color="neutral"
                                    size="sm"
                                    square
                                    :aria-label="$t('settings.deleteAria')"
                                    @click.stop="$emit('requestDelete', entry)"
                                />
                            </div>
                        </div>
                        <div v-if="layoutMode === 'manual' && manualActiveLayoutId !== entry.id" class="flex items-center justify-end" @click.stop>
                            <UButton
                                size="sm"
                                color="primary"
                                variant="outline"
                                @click.stop="entryActivateManual(entry.id)"
                            >
                                {{ $t('rules.activateBtn') }}
                            </UButton>
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
            </ul>
        </div>

        <LayoutConditionsModal
            v-if="modalLayoutId"
            v-model:open="modalOpen"
            :layout-id="modalLayoutId"
            :kind="modalKind"
            :layout-label="modalLayoutLabel"
        />
    </UCard>
</template>
