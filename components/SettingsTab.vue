<script setup lang="ts">
import MapperCard from '~/components/features/settings/MapperCard.vue'
import GeneralCard from '~/components/features/settings/GeneralCard.vue'
import LayoutsLibraryCard from '~/components/features/settings/LayoutsLibraryCard.vue'
import ConfigPathCard from '~/components/features/settings/ConfigPathCard.vue'
const {
  config,
  configPath,
  currentLayoutId,
  isLayoutDirty,
  library,
  mapper,
  theme,
  appLocale,
  appearanceItems,
  localeItems,
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
  performSave,
  closeSaveModal,
  deletePending,
  deleteBusy,
  confirmDelete,
  clearDeletePending,
  deviceOptions,
  selectedDevice,
  toggleMapper,
} = useSettingsScreen()
const { t } = useI18n()
</script>

<template>
  <div class="space-y-4">
    <MapperCard
      v-model:selected-device="selectedDevice"
      :mapper="mapper"
      :device-options="deviceOptions"
      @toggle="toggleMapper"
    />

    <GeneralCard
      :config="config"
      v-model:theme-preference="theme.preference.value"
      :resolved-theme="theme.resolved.value"
      v-model:locale-preference="appLocale.preference.value"
      :appearance-items="appearanceItems"
      :locale-items="localeItems"
    />

    <LayoutsLibraryCard
      :entries="library.entries.value"
      :current-layout-id="currentLayoutId"
      :is-layout-dirty="isLayoutDirty"
      :applying="applying"
      :apply-error="applyError"
      :layouts-dir="library.layoutsDir.value"
      @save-current="openSaveModal"
      @request-apply-entry="requestApplyEntry"
      @request-apply-empty="requestApplyEmpty"
      @request-delete="(entry) => deletePending = entry"
    />

    <ConfigPathCard :config-path="configPath" />

    <!-- Apply confirmation -->
    <UModal
      :open="!!pendingApply"
      :title="$t('settings.confirmApplyTitle', { label: pendingApply?.label ?? '' })"
      @update:open="(v) => !v && cancelApply()"
    >
      <template #body>
        <div class="space-y-3 text-sm">
          <p>
            {{ $t('settings.confirmApplyBody') }}
          </p>
          <div
            v-if="isLayoutDirty"
            class="flex items-start gap-2 p-3 rounded border border-(--ui-error)/40 bg-(--ui-error)/10"
          >
            <UIcon
              name="i-lucide-alert-triangle"
              class="text-(--ui-error) mt-0.5 shrink-0"
            />
            <div>
              <div class="font-semibold">
                {{ $t('settings.dirtyWarnTitle') }}
              </div>
              <div>
                <i18n-t keypath="settings.dirtyWarnBody" tag="span">
                  <template #btn>
                    <b>«{{ $t('settings.saveCurrent') }}»</b>
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
            {{ $t('common.cancel') }}
          </UButton>
          <UButton
            :color="isLayoutDirty ? 'error' : 'primary'"
            :loading="!!applying"
            @click="confirmApply"
          >
            {{ isLayoutDirty ? $t('settings.loseAndSwitch') : $t('settings.switch') }}
          </UButton>
        </div>
      </template>
    </UModal>

    <!-- Save-as modal -->
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
                <code class="break-all">{{ library.layoutsDir.value }}/{{ saveName || '…' }}.yaml</code>
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
            {{ $t('common.cancel') }}
          </UButton>
          <UButton
            color="primary"
            icon="i-lucide-save"
            :loading="saveBusy"
            :disabled="!saveName.trim()"
            @click="performSave"
          >
            {{ $t('common.save') }}
          </UButton>
        </div>
      </template>
    </UModal>

    <!-- Delete confirmation -->
    <UModal
      :open="!!deletePending"
      :title="$t('settings.deleteTitle', { name: deletePending?.name ?? '' })"
      @update:open="(v) => !v && clearDeletePending()"
    >
      <template #body>
        <p class="text-sm">
          {{ $t('settings.deleteBody') }}
        </p>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton
            variant="ghost"
            color="neutral"
            @click="clearDeletePending"
          >
            {{ $t('common.cancel') }}
          </UButton>
          <UButton
            color="error"
            :loading="deleteBusy"
            @click="confirmDelete"
          >
            {{ $t('common.delete') }}
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
