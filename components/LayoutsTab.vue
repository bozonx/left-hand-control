<script setup lang="ts">
import LayoutsLibraryCard from "~/components/features/settings/LayoutsLibraryCard.vue";

const {
    currentLayoutId,
    isLayoutDirty,
    library,
    applying,
    applyError,
    pendingApply,
    requestApplyEntry,
    requestApplyEmpty,
    cancelApply,
    confirmApply,
    saveModalOpen,
    saveName,
    saveBusy,
    saveError,
    openSaveModal,
    openSaveAsModal,
    performSave,
    closeSaveModal,
    deletePending,
    deleteBusy,
    confirmDelete,
    clearDeletePending,
} = useSettingsScreen();
</script>

<template>
    <div class="mx-auto w-full max-w-5xl space-y-4">
        <LayoutsLibraryCard
            :entries="library.entries.value"
            :current-layout-id="currentLayoutId"
            :is-layout-dirty="isLayoutDirty"
            :applying="applying"
            :apply-error="applyError"
            :library-error="library.error.value"
            :layouts-dir="library.layoutsDir.value"
            @save-current="openSaveModal"
            @save-as="openSaveAsModal"
            @request-apply-entry="requestApplyEntry"
            @request-apply-empty="requestApplyEmpty"
            @request-delete="(entry) => (deletePending = entry)"
        />

        <UModal
            :open="!!pendingApply"
            :title="
                $t('settings.confirmApplyTitle', {
                    label: pendingApply?.label ?? '',
                })
            "
            @update:open="(v) => !v && cancelApply()"
        >
            <template #body>
                <div class="space-y-3 text-sm">
                    <p>
                        {{ $t("settings.confirmApplyBody") }}
                    </p>
                    <div
                        v-if="isLayoutDirty"
                        class="flex items-start gap-2 p-3 rounded border border-(--ui-error)/40 bg-(--ui-error)/10"
                    >
                        <UIcon
                            name="i-lucide-triangle-alert"
                            class="text-(--ui-error) mt-0.5 shrink-0"
                        />
                        <div>
                            <div class="font-semibold">
                                {{ $t("settings.dirtyWarnTitle") }}
                            </div>
                            <div>
                                <i18n-t
                                    keypath="settings.dirtyWarnBody"
                                    tag="span"
                                >
                                    <template #btn>
                                        <b
                                            >«{{
                                                $t("settings.saveCurrent")
                                            }}»</b
                                        >
                                    </template>
                                </i18n-t>
                            </div>
                        </div>
                    </div>
                </div>
            </template>
            <template #footer>
                <div class="flex gap-2 justify-end w-full">
                    <UButton
                        variant="ghost"
                        color="neutral"
                        @click="cancelApply"
                    >
                        {{ $t("common.cancel") }}
                    </UButton>
                    <UButton
                        :color="isLayoutDirty ? 'error' : 'primary'"
                        :loading="!!applying"
                        @click="confirmApply"
                    >
                        {{
                            isLayoutDirty
                                ? $t("settings.loseAndSwitch")
                                : $t("settings.switch")
                        }}
                    </UButton>
                </div>
            </template>
        </UModal>

        <UModal
            v-model:open="saveModalOpen"
            :title="$t('settings.saveModalTitle')"
        >
            <template #body>
                <div class="space-y-3">
                    <UFormField :label="$t('settings.nameLabel')">
                        <UInput
                            v-model="saveName"
                            :placeholder="$t('settings.namePh')"
                            autofocus
                            @keyup.enter="performSave"
                        />
                    </UFormField>
                    <p class="text-xs text-(--ui-text-muted)">
                        <i18n-t keypath="settings.saveHint" tag="span">
                            <template #path>
                                <code class="break-all"
                                    >{{ library.layoutsDir.value }}/{{
                                        saveName || "..."
                                    }}.yaml</code
                                >
                            </template>
                        </i18n-t>
                    </p>
                    <p v-if="saveError" class="text-sm text-(--ui-error)">
                        {{ saveError }}
                    </p>
                </div>
            </template>
            <template #footer>
                <div class="flex gap-2 justify-end w-full">
                    <UButton
                        variant="ghost"
                        color="neutral"
                        @click="closeSaveModal"
                    >
                        {{ $t("common.cancel") }}
                    </UButton>
                    <UButton
                        color="primary"
                        icon="i-lucide-save"
                        :loading="saveBusy"
                        :disabled="!saveName.trim()"
                        @click="performSave"
                    >
                        {{ $t("common.save") }}
                    </UButton>
                </div>
            </template>
        </UModal>

        <UModal
            :open="!!deletePending"
            :title="
                $t('settings.deleteTitle', { name: deletePending?.name ?? '' })
            "
            @update:open="(v) => !v && clearDeletePending()"
        >
            <template #body>
                <p class="text-sm">
                    {{ $t("settings.deleteBody") }}
                </p>
            </template>
            <template #footer>
                <div class="flex gap-2 justify-end w-full">
                    <UButton
                        variant="ghost"
                        color="neutral"
                        @click="clearDeletePending"
                    >
                        {{ $t("common.cancel") }}
                    </UButton>
                    <UButton
                        color="error"
                        :loading="deleteBusy"
                        @click="confirmDelete"
                    >
                        {{ $t("common.delete") }}
                    </UButton>
                </div>
            </template>
        </UModal>
    </div>
</template>
