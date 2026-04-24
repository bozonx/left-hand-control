<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import MacroEditorCard from '~/components/features/macros/MacroEditorCard.vue'
import SystemMacrosCard from '~/components/features/macros/SystemMacrosCard.vue'
import { SYSTEM_MACROS, type SystemMacro } from '~/utils/systemMacros'

const { config } = useConfig()
const {
  addMacro,
  cloneSystemMacro,
  removeMacro,
  moveMacro,
  addStep,
  removeStep,
  moveStep,
  uiKeyOf,
  idError,
  hasErrors,
  stepError,
  usage,
} = useMacroEditor()

const emit = defineEmits<{
  backToTop: []
}>()

const systemOpen = ref(false)
const focusMacroKey = ref<string | null>(null)

function macroNameInputId(uiKey: string) {
  return `macro-name-${uiKey}`
}

async function focusMacroName(uiKey: string) {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    await nextTick()
    await new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)))
    const input = document.getElementById(macroNameInputId(uiKey)) as HTMLInputElement | null
    if (!input) continue
    input.focus()
    input.select()
    if (document.activeElement === input) return
  }
}

async function createMacro() {
  const macro = addMacro()
  const uiKey = uiKeyOf(macro)
  focusMacroKey.value = uiKey
  await focusMacroName(uiKey)
}

async function createFromSystemMacro(sys: SystemMacro) {
  const macro = cloneSystemMacro(sys)
  const uiKey = uiKeyOf(macro)
  focusMacroKey.value = uiKey
  emit('backToTop')
  await focusMacroName(uiKey)
}

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
          <AppTooltip :disabled="!hasErrors" :text="$t('macros.addDisabled')">
            <UButton
              icon="i-lucide-plus"
              size="sm"
              class="whitespace-nowrap"
              :disabled="hasErrors"
              @click="createMacro"
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
          v-for="(macro, index) in config.macros"
          :key="uiKeyOf(macro)"
          :ui-key="uiKeyOf(macro)"
          :macro="macro"
          :is-first="index === 0"
          :is-last="index === config.macros.length - 1"
          :name-input-id="macroNameInputId(uiKeyOf(macro))"
          :id-error="idError(macro) ?? undefined"
          :step-error="stepError"
          :usage="usage[macro.id] ?? []"
          :default-step-pause-ms="config.settings.defaultMacroStepPauseMs"
          :default-modifier-delay-ms="config.settings.defaultMacroModifierDelayMs"
          @remove="askRemove"
          @move-up="moveMacro($event, -1)"
          @move-down="moveMacro($event, 1)"
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
      @clone="createFromSystemMacro"
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
