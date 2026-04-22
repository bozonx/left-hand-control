<script setup lang="ts">
import MacroEditorCard from '~/components/features/macros/MacroEditorCard.vue'
import SystemMacrosCard from '~/components/features/macros/SystemMacrosCard.vue'
import { SYSTEM_MACROS } from '~/utils/systemMacros'

const { config } = useConfig()
const {
  addMacro,
  cloneSystemMacro,
  removeMacro,
  addStep,
  removeStep,
  moveStep,
  uiKeyOf,
  idError,
  hasIdErrors,
  usage,
} = useMacroEditor()

const systemOpen = ref(false)

// --- Deletion confirmation -----------------------------------------------
const confirmOpen = ref(false)
const pendingDeleteKey = ref<string | null>(null)
const pendingDeleteLabel = ref<string | null>(null)

function askRemove(payload: { uiKey: string, id: string }) {
  pendingDeleteKey.value = payload.uiKey
  pendingDeleteLabel.value = payload.id
  confirmOpen.value = true
}

function confirmRemove() {
  if (pendingDeleteKey.value) removeMacro(pendingDeleteKey.value)
  pendingDeleteKey.value = null
  pendingDeleteLabel.value = null
  confirmOpen.value = false
}

function cancelRemove() {
  confirmOpen.value = false
  pendingDeleteKey.value = null
  pendingDeleteLabel.value = null
}
</script>

<template>
  <div class="space-y-4">
    <UCard>
      <template #header>
        <div class="flex items-center justify-between gap-3">
          <div>
            <h2 class="text-sm font-semibold">{{ $t('macros.title') }}</h2>
            <p class="text-xs text-(--ui-text-muted) mt-0.5">
              {{ $t('macros.subtitle') }}
            </p>
          </div>
          <AppTooltip :disabled="!hasIdErrors" :text="$t('macros.addDisabled')">
            <UButton
              icon="i-lucide-plus"
              size="sm"
              :disabled="hasIdErrors"
              @click="addMacro"
            >
              {{ $t('macros.addBtn') }}
            </UButton>
          </AppTooltip>
        </div>
      </template>

      <div class="space-y-4">
        <div
          v-if="config.macros.length === 0"
          class="text-sm text-(--ui-text-muted)"
        >
          {{ $t('macros.empty') }}
        </div>

        <MacroEditorCard
          v-for="macro in config.macros"
          :key="uiKeyOf(macro)"
          :ui-key="uiKeyOf(macro)"
          :macro="macro"
          :id-error="idError(macro) ?? undefined"
          :usage="usage[macro.id] ?? []"
          :default-step-pause-ms="config.settings.defaultMacroStepPauseMs"
          :default-modifier-delay-ms="config.settings.defaultMacroModifierDelayMs"
          @remove="askRemove"
          @add-step="addStep"
          @move-step="moveStep"
          @remove-step="removeStep"
        />
      </div>
    </UCard>

    <SystemMacrosCard
      v-model:system-open="systemOpen"
      :usage="usage"
      :system-macros="SYSTEM_MACROS"
      @clone="cloneSystemMacro"
    />

    <UModal v-model:open="confirmOpen" :title="$t('macros.confirmDeleteTitle')">
      <template #body>
        <p class="text-sm">
          <i18n-t keypath="macros.confirmDeleteBody" tag="span">
            <template #ref>
              <code>macro:{{ pendingDeleteLabel }}</code>
            </template>
          </i18n-t>
        </p>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton variant="ghost" color="neutral" @click="cancelRemove">
            {{ $t('common.cancel') }}
          </UButton>
          <UButton color="error" icon="i-lucide-trash-2" @click="confirmRemove">
            {{ $t('common.delete') }}
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
