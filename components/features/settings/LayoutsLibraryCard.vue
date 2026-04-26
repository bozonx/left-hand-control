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
}>();

const { config } = useConfig();
const { t } = useI18n();
const { setIncludedInAuto, setAsDefault } = useLayoutConditions();

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

function entryHasConditions(entryId: string) {
    const rule = config.value.settings.layoutConditions[entryId];
    return !!(rule?.whitelist || rule?.blacklist);
}

function entryIsIncluded(entryId: string) {
    return props.autoIncludedIds.has(entryId);
}

function entryToggleDefault(entryId: string, value: boolean) {
    setAsDefault(value ? entryId : undefined);
}

function entryToggleAuto(entryId: string, value: boolean) {
    setIncludedInAuto(entryId, value);
}

function entryActivateManual(entryId: string) {
    config.value.settings.manualActiveLayoutId = entryId;
}

function summarize(set: LayoutConditionSet | undefined): string {
    if (!set) return t("rules.conditionsNone");
    const parts: string[] = [];
    if (set.gameMode === "on") parts.push(t("rules.gameModeOn"));
    else if (set.gameMode === "off") parts.push(t("rules.gameModeOff"));
    if (set.layouts.length > 0) parts.push(set.layouts.join(", "));
    return parts.length > 0 ? parts.join(" · ") : t("rules.conditionsNone");
}

function entryWhitelistSummary(entryId: string) {
    return summarize(config.value.settings.layoutConditions[entryId]?.whitelist);
}

function entryBlacklistSummary(entryId: string) {
    return summarize(config.value.settings.layoutConditions[entryId]?.blacklist);
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
                    class="flex items-center justify-between gap-3 p-3 group hover:bg-(--ui-bg-elevated) transition-colors cursor-pointer"
                    @click="$emit('requestApplyEntry', entry)"
                >
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
                        <div class="min-w-0">
                            <div
                                class="font-medium truncate flex items-center gap-2 flex-wrap"
                            >
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
                                    v-if="
                                        currentLayoutId === entry.id
                                    "
                                    color="info"
                                    variant="subtle"
                                    size="sm"
                                >
                                    Editing
                                </UBadge>
                                <UBadge
                                    v-if="
                                        currentLayoutId === entry.id &&
                                        isLayoutDirty
                                    "
                                    color="warning"
                                    variant="subtle"
                                    size="sm"
                                >
                                    {{ $t("settings.unsavedBadge") }}
                                </UBadge>
                            </div>
                            <div
                                v-if="
                                    (currentLayoutId === entry.id
                                        ? currentLayoutDescription
                                        : entry.description) ||
                                    false
                                "
                                class="text-sm text-(--ui-text-muted) line-clamp-2 mt-0.5"
                            >
                                {{
                                    currentLayoutId === entry.id
                                        ? currentLayoutDescription
                                        : entry.description
                                }}
                            </div>
                            <div v-if="layoutMode === 'auto'" class="flex items-center gap-3 mt-1 flex-wrap" @click.stop>
                                <div class="flex items-center gap-1.5 cursor-pointer">
                                    <USwitch
                                        :model-value="entryIsDefault(entry.id)"
                                        @update:model-value="entryToggleDefault(entry.id, $event === true)"
                                    />
                                    <span class="text-xs text-(--ui-text-muted) select-none">{{ $t('rules.autoDefaultLabel') }}</span>
                                </div>
                                <div class="flex items-center gap-1.5 cursor-pointer">
                                    <USwitch
                                        :model-value="entryIsIncluded(entry.id)"
                                        :disabled="entryHasConditions(entry.id) || entryIsDefault(entry.id)"
                                        @update:model-value="entryToggleAuto(entry.id, $event === true)"
                                    />
                                    <span class="text-xs text-(--ui-text-muted) select-none">{{ $t('rules.autoIncludeLabel') }}</span>
                                </div>
                            </div>
                            <div v-if="layoutMode === 'auto'" class="flex items-center gap-2 mt-1 flex-wrap" @click.stop>
                                <UButton
                                    size="xs"
                                    color="neutral"
                                    variant="outline"
                                    :disabled="entryIsDefault(entry.id)"
                                    @click="openWhitelist(entry.id)"
                                >
                                    <div class="flex items-center gap-1 min-w-0">
                                        <UIcon name="i-lucide-list-checks" class="shrink-0" />
                                        <span class="truncate">{{ entryWhitelistSummary(entry.id) }}</span>
                                    </div>
                                </UButton>
                                <UButton
                                    size="xs"
                                    color="neutral"
                                    variant="outline"
                                    :disabled="entryIsDefault(entry.id)"
                                    @click="openBlacklist(entry.id)"
                                >
                                    <div class="flex items-center gap-1 min-w-0">
                                        <UIcon name="i-lucide-list-x" class="shrink-0" />
                                        <span class="truncate">{{ entryBlacklistSummary(entry.id) }}</span>
                                    </div>
                                </UButton>
                            </div>
                            <div v-if="layoutMode === 'manual' && manualActiveLayoutId !== entry.id" class="mt-1" @click.stop>
                                <UButton
                                    size="xs"
                                    color="primary"
                                    variant="outline"
                                    @click="entryActivateManual(entry.id)"
                                >
                                    {{ $t('rules.activateBtn') }}
                                </UButton>
                            </div>
                        </div>
                    </div>
                    <div
                        class="flex items-center gap-2 shrink-0 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity"
                        @click.stop
                    >
                        <UButton
                            color="neutral"
                            variant="ghost"
                            icon="i-lucide-pencil"
                            :aria-label="$t('settings.editLayoutAria', { name: entry.name })"
                            @click="$emit('requestEdit', entry)"
                        />
                        <UButton
                            color="neutral"
                            variant="ghost"
                            icon="i-lucide-trash-2"
                            :aria-label="$t('settings.deleteAria')"
                            @click="$emit('requestDelete', entry)"
                        />
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
