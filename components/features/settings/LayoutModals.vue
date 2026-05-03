<script setup lang="ts">
const {
  isLayoutDirty,
  library,
  applying,
  applyError: _applyError,
  pendingApply,
  cancelApply,
  confirmApply,
  saveModalOpen,
  saveName,
  saveBusy,
  saveError,
  performSave,
  closeSaveModal,
  editModalOpen,
  editName,
  editDescription,
  editMode,
  editBusy,
  editError,
  performEdit,
  closeEditModal,
  overwriteConfirmOpen,
  overwriteTargetName,
  confirmOverwrite,
  closeOverwriteConfirm,
  resetConfirmOpen,
  resetBusy,
  confirmReset,
  closeResetConfirm,
  deletePending,
  deleteBusy,
  confirmDelete,
  clearDeletePending,
} = useSettingsScreen();
const { t } = useI18n()

const editTitle = computed(() => {
  if (editMode.value === 'name') return t('settings.renameLayoutTitle')
  if (editMode.value === 'description') return t('settings.editDescriptionTitle')
  return t('settings.editLayoutTitle')
})
</script>

<template>
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
              <i18n-t keypath="settings.dirtyWarnBody" tag="span">
                <template #btn>
                  <b>«{{ $t("settings.saveCurrent") }}»</b>
                </template>
              </i18n-t>
            </div>
          </div>
        </div>
      </div>
    </template>
    <template #footer>
      <div class="flex gap-2 justify-end w-full">
        <UButton variant="ghost" color="neutral" @click="cancelApply">
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

  <UModal v-model:open="saveModalOpen" :title="$t('settings.saveModalTitle')">
    <template #body>
      <div class="space-y-3">
        <UFormField :label="$t('settings.nameLabel')">
          <UInput
            v-model="saveName"
            class="w-full"
            :placeholder="$t('settings.namePh')"
            autofocus
            @keyup.enter="performSave(false)"
          />
        </UFormField>
        <p class="text-xs text-(--ui-text-muted)">
          <i18n-t keypath="settings.saveHint" tag="span">
            <template #path>
              <code class="break-all"
                >{{ library.layoutsDir.value }}/{{ saveName || "..." }}.yaml</code
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
        <UButton variant="ghost" color="neutral" @click="closeSaveModal">
          {{ $t("common.cancel") }}
        </UButton>
        <UButton
          color="primary"
          icon="i-lucide-save"
          :loading="saveBusy"
          :disabled="!saveName.trim()"
          @click="performSave(false)"
        >
          {{ $t("common.save") }}
        </UButton>
      </div>
    </template>
  </UModal>

  <UModal v-model:open="editModalOpen" :title="editTitle">
    <template #body>
      <div class="space-y-3">
        <UFormField v-if="editMode !== 'description'" :label="$t('settings.nameLabel')">
          <UInput
            v-model="editName"
            class="w-full"
            :placeholder="$t('settings.namePh')"
            autofocus
            @keyup.enter="performEdit(false)"
          />
        </UFormField>
        <UFormField v-if="editMode !== 'name'" :label="$t('settings.descriptionLabel')">
          <UTextarea
            v-model="editDescription"
            class="w-full"
            :placeholder="$t('settings.descriptionPh')"
            :rows="4"
          />
        </UFormField>
        <p v-if="editMode !== 'description'" class="text-xs text-(--ui-text-muted)">
          <i18n-t keypath="settings.saveHint" tag="span">
            <template #path>
              <code class="break-all"
                >{{ library.layoutsDir.value }}/{{ editName || "..." }}.yaml</code
              >
            </template>
          </i18n-t>
        </p>
        <p v-if="editError" class="text-sm text-(--ui-error)">
          {{ editError }}
        </p>
      </div>
    </template>
    <template #footer>
      <div class="flex gap-2 justify-end w-full">
        <UButton variant="ghost" color="neutral" @click="closeEditModal">
          {{ $t("common.cancel") }}
        </UButton>
        <UButton
          color="primary"
          icon="i-lucide-save"
          :loading="editBusy"
          :disabled="!editName.trim()"
          @click="performEdit(false)"
        >
          {{ $t("common.save") }}
        </UButton>
      </div>
    </template>
  </UModal>

  <UModal
    :open="overwriteConfirmOpen"
    :title="$t('settings.overwriteTitle', { name: overwriteTargetName })"
    @update:open="(v) => !v && closeOverwriteConfirm()"
  >
    <template #body>
      <p class="text-sm">
        {{ $t("settings.overwriteBody") }}
      </p>
    </template>
    <template #footer>
      <div class="flex gap-2 justify-end w-full">
        <UButton variant="ghost" color="neutral" @click="closeOverwriteConfirm">
          {{ $t("common.cancel") }}
        </UButton>
        <UButton color="error" @click="confirmOverwrite">
          {{ $t("common.override") }}
        </UButton>
      </div>
    </template>
  </UModal>

  <UModal
    :open="resetConfirmOpen"
    :title="$t('settings.resetUnsavedTitle')"
    @update:open="(v) => !v && closeResetConfirm()"
  >
    <template #body>
      <p class="text-sm">
        {{ $t("settings.resetUnsavedBody") }}
      </p>
    </template>
    <template #footer>
      <div class="flex gap-2 justify-end w-full">
        <UButton variant="ghost" color="neutral" @click="closeResetConfirm">
          {{ $t("common.cancel") }}
        </UButton>
        <UButton color="error" :loading="resetBusy" @click="confirmReset">
          {{ $t("settings.resetUnsavedBtn") }}
        </UButton>
      </div>
    </template>
  </UModal>

  <UModal
    :open="!!deletePending"
    :title="$t('settings.deleteTitle', { name: deletePending?.name ?? '' })"
    @update:open="(v) => !v && clearDeletePending()"
  >
    <template #body>
      <p class="text-sm">
        {{ $t("settings.deleteBody") }}
      </p>
    </template>
    <template #footer>
      <div class="flex gap-2 justify-end w-full">
        <UButton variant="ghost" color="neutral" @click="clearDeletePending">
          {{ $t("common.cancel") }}
        </UButton>
        <UButton color="error" :loading="deleteBusy" @click="confirmDelete">
          {{ $t("common.delete") }}
        </UButton>
      </div>
    </template>
  </UModal>
</template>
