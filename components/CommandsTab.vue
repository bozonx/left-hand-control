<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import CommandEditorCard from '~/components/features/commands/CommandEditorCard.vue'

const { config } = useConfig()
const {
  addCommand,
  removeCommand,
  moveCommand,
  uiKeyOf,
  idError,
  linuxError,
  hasErrors,
  usage,
} = useCommandEditor()

const commandIds = computed(() => config.value.commands.map((c) => uiKeyOf(c)))
const { selectedId, select, containerRef } = useListKeyboardNavigation({
  ids: commandIds,
  move: moveCommand,
})

const emit = defineEmits<{
  backToTop: []
}>()

const confirmOpen = ref(false)
const pendingDeleteKey = ref<string | null>(null)
const pendingDeleteLabel = ref<string | null>(null)
const focusCommandKey = ref<string | null>(null)

function commandNameInputId(uiKey: string) {
  return `command-name-${uiKey}`
}

function clearFocusCommandKey(uiKey: string) {
  if (focusCommandKey.value === uiKey) focusCommandKey.value = null
}

async function createCommand() {
  const command = addCommand()
  const uiKey = uiKeyOf(command)
  focusCommandKey.value = uiKey
  emit('backToTop')
}

function askRemove(payload: { uiKey: string, id: string }) {
  pendingDeleteKey.value = payload.uiKey
  pendingDeleteLabel.value = payload.id
  confirmOpen.value = true
}

function confirmRemove() {
  if (pendingDeleteKey.value) removeCommand(pendingDeleteKey.value)
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
            <h2 class="text-sm font-semibold">{{ $t('commands.title') }}</h2>
            <p class="mt-0.5 text-xs text-(--ui-text-muted)">
              {{ $t('commands.subtitle') }}
            </p>
          </div>
          <AppTooltip :disabled="!hasErrors" :text="$t('commands.addDisabled')">
            <UButton
              icon="i-lucide-plus"
              size="sm"
              class="whitespace-nowrap"
              :disabled="hasErrors"
              @click="createCommand"
            >
              {{ $t('commands.addBtn') }}
            </UButton>
          </AppTooltip>
        </div>
      </template>

      <div ref="containerRef" class="space-y-4">
        <div
          v-if="config.commands.length === 0"
          class="text-sm text-(--ui-text-muted)"
        >
          {{ $t('commands.empty') }}
        </div>

        <CommandEditorCard
          v-for="(command, index) in config.commands"
          :key="uiKeyOf(command)"
          :ui-key="uiKeyOf(command)"
          :command="command"
          :name-input-id="commandNameInputId(uiKeyOf(command))"
          :usage="usage[command.id] ?? []"
          :id-error="idError(command) ?? undefined"
          :linux-error="linuxError(command) ?? undefined"
          :is-first="index === 0"
          :is-last="index === config.commands.length - 1"
          :focus-name="uiKeyOf(command) === focusCommandKey"
          :selected="selectedId === uiKeyOf(command)"
          @select="select(uiKeyOf(command))"
          @remove="askRemove"
          @move-up="moveCommand($event, -1)"
          @move-down="moveCommand($event, 1)"
          @name-focused="clearFocusCommandKey"
        />
      </div>
    </UCard>

    <UModal v-model:open="confirmOpen" :title="$t('commands.confirmDeleteTitle')">
      <template #body>
        <p class="text-sm">
          <i18n-t keypath="commands.confirmDeleteBody" tag="span">
            <template #ref>
              <code>cmd:{{ pendingDeleteLabel }}</code>
            </template>
          </i18n-t>
        </p>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2">
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
