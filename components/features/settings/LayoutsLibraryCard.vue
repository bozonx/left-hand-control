<script setup lang="ts">
import { computed, ref } from "vue";
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";
import { isUserLayoutId, userLayoutNameFromId } from "~/composables/useLayoutLibrary";
import type { LayoutMode } from "~/types/config";
import type { LayoutEditMode } from "~/composables/settingsScreen/useLayoutEdit";
import { useLayoutConditions } from "~/composables/useLayoutConditions";
import type { ConditionKind } from "~/composables/useLayoutConditions";
import LayoutConditionsModal from "~/components/features/settings/LayoutConditionsModal.vue";
import LayoutsLibraryItem from "~/components/features/settings/LayoutsLibraryItem.vue";

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
    activeAutoLayoutId?: string;
    manualActiveLayoutId?: string;
    selectedId?: string | null;
}>();

const _emit = defineEmits<{
    saveCurrent: [];
    saveAs: [];
    requestApplyEntry: [entry: LayoutLibraryEntry];
    createFromEmpty: [];
    createFromIvanK: [];
    requestEdit: [entry: LayoutLibraryEntry, mode?: LayoutEditMode];
    requestReset: [];
    requestDelete: [entry: LayoutLibraryEntry];
    moveUp: [entry: LayoutLibraryEntry];
    moveDown: [entry: LayoutLibraryEntry];
    select: [id: string];
}>();

const { config } = useConfig();
const { setEnabledInAuto } = useLayoutConditions();

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

function openWhitelist(entryId: string) {
    modalLayoutId.value = entryId;
    modalKind.value = "whitelist";
    modalOpen.value = true;
}

function openBlacklist(entryId: string) {
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

            <div role="alert">
                <p v-if="applyError" class="text-sm text-(--ui-error)">
                    {{ applyError }}
                </p>
                <p v-if="libraryError" class="text-sm text-(--ui-error)">
                    {{ libraryError }}
                </p>
            </div>

            <ul class="space-y-3">
                <LayoutsLibraryItem
                    v-for="(entry, index) in entries"
                    :key="entry.id"
                    :entry="entry"
                    :index="index"
                    :is-first="index === 0"
                    :is-last="index === entries.length - 1"
                    :layout-mode="layoutMode"
                    :current-layout-id="currentLayoutId"
                    :current-layout-description="currentLayoutDescription"
                    :is-layout-dirty="isLayoutDirty"
                    :applying="applying"
                    :active-auto-layout-id="activeAutoLayoutId"
                    :manual-active-layout-id="manualActiveLayoutId"
                    :auto-included-ids="autoIncludedIds"
                    :selected-id="selectedId"
                    @select="$emit('select', $event)"
                    @request-edit="$emit('requestEdit', $event, 'name')"
                    @request-edit-description="$emit('requestEdit', $event, 'description')"
                    @request-apply-entry="$emit('requestApplyEntry', $event)"
                    @request-delete="$emit('requestDelete', $event)"
                    @move-up="$emit('moveUp', $event)"
                    @move-down="$emit('moveDown', $event)"
                    @activate-manual="config.settings.manualActiveLayoutId = $event"
                    @toggle-auto="(entryId, value) => setEnabledInAuto(entryId, value)"
                    @open-whitelist="openWhitelist"
                    @open-blacklist="openBlacklist"
                />
            </ul>

            <div v-if="!entries.length" class="py-8 text-center text-sm text-(--ui-text-muted)">
                <p class="font-medium text-(--ui-text-highlighted)">{{ $t('settings.emptyLayoutsTitle') }}</p>
                <p class="mt-1">{{ $t('settings.emptyLayoutsBody') }}</p>
            </div>
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
