<script setup lang="ts">
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import type { LayoutMode } from "~/types/config";
import { useLayoutConditions } from "~/composables/useLayoutConditions";

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
const { setIncludedInAuto, setAsDefault } = useLayoutConditions();

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
                                <label class="flex items-center gap-1.5 cursor-pointer">
                                    <UToggle
                                        :model-value="entryIsDefault(entry.id)"
                                        @update:model-value="entryToggleDefault(entry.id, $event === true)"
                                    />
                                    <span class="text-xs text-(--ui-text-muted) select-none">{{ $t('rules.autoDefaultLabel') }}</span>
                                </label>
                                <label class="flex items-center gap-1.5 cursor-pointer">
                                    <UToggle
                                        :model-value="entryIsIncluded(entry.id)"
                                        :disabled="entryHasConditions(entry.id) || entryIsDefault(entry.id)"
                                        @update:model-value="entryToggleAuto(entry.id, $event === true)"
                                    />
                                    <span class="text-xs text-(--ui-text-muted) select-none">{{ $t('rules.autoIncludeLabel') }}</span>
                                </label>
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
    </UCard>
</template>
