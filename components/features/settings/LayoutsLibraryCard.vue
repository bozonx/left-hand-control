<script setup lang="ts">
import type { LayoutLibraryEntry } from "~/composables/useLayoutLibrary";

defineProps<{
    entries: LayoutLibraryEntry[];
    currentLayoutId?: string;
    isLayoutDirty: boolean;
    applying: string;
    applyError?: string | null;
    libraryError?: string | null;
    layoutsDir: string;
}>();

defineEmits<{
    saveCurrent: [];
    saveAs: [];
    requestApplyEntry: [entry: LayoutLibraryEntry];
    requestApplyEmpty: [];
    requestDelete: [entry: LayoutLibraryEntry];
}>();
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
                        :loading="applying === 'empty'"
                        :disabled="!!applying"
                        @click="$emit('requestApplyEmpty')"
                    >
                        {{ $t("settings.newLayoutBtn") }}
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
                    <UButton
                        color="primary"
                        icon="i-lucide-save"
                        :disabled="!isLayoutDirty && !currentLayoutId"
                        @click="$emit('saveCurrent')"
                    >
                        {{ $t("settings.saveCurrent") }}
                    </UButton>
                </div>
            </div>
        </template>

        <div class="space-y-3">
            <div
                v-if="isLayoutDirty"
                class="flex items-start gap-2 p-3 rounded border border-(--ui-warning)/40 bg-(--ui-warning)/10 text-sm"
            >
                <UIcon
                    name="i-lucide-triangle-alert"
                    class="text-(--ui-warning) mt-0.5 shrink-0"
                />
                <div>
                    <div class="font-semibold">
                        {{ $t("settings.dirtyBadgeTitle") }}
                    </div>
                    <div class="text-(--ui-text-muted)">
                        {{ $t("settings.dirtyBadgeBody") }}
                    </div>
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
                    v-for="entry in entries"
                    :key="entry.id"
                    class="flex items-center justify-between gap-3 p-3 group hover:bg-(--ui-bg-elevated) transition-colors"
                >
                    <div class="flex items-center gap-2 min-w-0">
                        <UIcon
                            :name="
                                entry.builtin
                                    ? 'i-lucide-sparkles'
                                    : 'i-lucide-file'
                            "
                            :class="entry.builtin ? 'text-(--ui-primary)' : ''"
                        />
                        <div class="min-w-0">
                            <div
                                class="font-medium truncate flex items-center gap-2"
                            >
                                {{ entry.name }}
                                <UBadge
                                    v-if="
                                        currentLayoutId === entry.id &&
                                        !isLayoutDirty
                                    "
                                    color="success"
                                    variant="subtle"
                                    size="sm"
                                >
                                    {{ $t("settings.activeBadge") }}
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
                                <UBadge
                                    v-if="entry.builtin"
                                    color="primary"
                                    variant="outline"
                                    size="sm"
                                >
                                    {{ $t("settings.builtinBadge") }}
                                </UBadge>
                            </div>
                        </div>
                    </div>
                    <div
                        class="flex items-center gap-2 shrink-0 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity"
                    >
                        <UButton
                            variant="outline"
                            icon="i-lucide-rotate-ccw"
                            :loading="applying === entry.id"
                            :disabled="!!applying"
                            @click="$emit('requestApplyEntry', entry)"
                        >
                            {{ $t("settings.applyBtn") }}
                        </UButton>
                        <UButton
                            v-if="!entry.builtin"
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
