<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import type { Command } from '~/types/config'

const props = defineProps<{
  command: Command
  uiKey: string
  nameInputId: string
  usage: string[]
  idError?: string
  linuxError?: string
  isFirst?: boolean
  isLast?: boolean
  selected?: boolean
  focusName?: boolean
}>()

const emit = defineEmits<{
  select: []
  remove: [payload: { uiKey: string, id: string }]
  moveUp: [uiKey: string]
  moveDown: [uiKey: string]
  nameFocused: [uiKey: string]
}>()

function onCardClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null
  if (target?.closest('input, textarea, select, button, [role="dialog"], [role="listbox"]')) return
  emit('select')
}

const { copy: copyToClipboard } = useClipboardCopy()
const nameInputRef = useTemplateRef<{ inputRef?: { value?: HTMLInputElement } }>('nameInputRef')

watch(
  () => props.focusName,
  async (value) => {
    if (!value) return
    await new Promise((resolve) => requestAnimationFrame(() => resolve(undefined)))
    const input = nameInputRef.value?.inputRef?.value
    if (!input) return
    input.focus()
    input.select()
    emit('nameFocused', props.uiKey)
  },
  { immediate: true, flush: 'post' },
)

async function copyCommandId() {
  if (!props.command.id) return
  await copyToClipboard(props.command.id)
}
</script>

<template>
  <div
    class="relative rounded-xl border p-4 transition-all duration-150 cursor-pointer"
    :class="[
      selected
        ? 'border-(--ui-primary) ring-1 ring-(--ui-primary) bg-(--ui-bg-muted)/60 shadow-lg shadow-(--ui-primary)/5'
        : 'border-(--ui-border) bg-(--ui-bg-muted)/40 hover:border-(--ui-primary)/50 hover:bg-(--ui-bg-muted)/60 hover:shadow-lg hover:shadow-(--ui-primary)/5',
    ]"
    @click="onCardClick"
  >
    <div class="flex items-start justify-between gap-4">
      <div class="grid flex-1 grid-cols-2 gap-3 min-w-0">
        <UFormField :error="idError">
          <template #label>
            <FieldLabel
              :label="$t('commands.idLabel')"
              :hint="$t('commands.idHint')"
            />
          </template>
          <div class="flex items-center gap-2">
            <UInput
              v-model="command.id"
              :color="idError ? 'error' : undefined"
              :highlight="!!idError"
              class="w-full font-mono"
              :placeholder="$t('commands.idPh')"
            />
            <AppTooltip :text="$t('commands.copyId')">
              <UButton
                icon="i-lucide-copy"
                size="sm"
                color="neutral"
                variant="ghost"
                :aria-label="$t('commands.copyId')"
                :disabled="!command.id"
                @click="copyCommandId"
              />
            </AppTooltip>
          </div>
        </UFormField>

        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('commands.nameLabel')"
              :hint="$t('commands.nameHint')"
            />
          </template>
          <UInput
            ref="nameInputRef"
            :id="nameInputId"
            v-model="command.name"
            :placeholder="$t('commands.namePh')"
            class="w-full"
          />
        </UFormField>
      </div>

      <div class="flex gap-1">
        <AppTooltip :text="$t('common.moveUp')">
          <UButton
            icon="i-lucide-arrow-up"
            variant="ghost"
            color="neutral"
            size="sm"
            square
            :disabled="isFirst"
            :aria-label="$t('commands.moveUp')"
            @click="$emit('moveUp', uiKey)"
          />
        </AppTooltip>
        <AppTooltip :text="$t('common.moveDown')">
          <UButton
            icon="i-lucide-arrow-down"
            variant="ghost"
            color="neutral"
            size="sm"
            square
            :disabled="isLast"
            :aria-label="$t('commands.moveDown')"
            @click="$emit('moveDown', uiKey)"
          />
        </AppTooltip>
        <AppTooltip :text="$t('commands.deleteCommand')">
          <UButton
            icon="i-lucide-trash-2"
            variant="ghost"
            color="error"
            size="sm"
            square
            :aria-label="$t('commands.deleteCommand')"
            @click="$emit('remove', { uiKey, id: command.id })"
          />
        </AppTooltip>
      </div>
    </div>

    <div
      v-if="usage.length"
      class="mt-3 flex flex-wrap gap-1 text-xs"
    >
      <span class="text-(--ui-text-muted)">{{ $t('commands.usedIn') }}</span>
      <UBadge
        v-for="place in usage"
        :key="place"
        color="neutral"
        variant="subtle"
        class="font-mono"
      >
        {{ place }}
      </UBadge>
    </div>

    <UFormField
      class="mt-4"
      :error="linuxError"
    >
      <template #label>
        <FieldLabel
          :label="$t('commands.linuxLabel')"
          :hint="$t('commands.linuxHint')"
        />
      </template>
      <UTextarea
        v-model="command.linux"
        autoresize
        :rows="3"
        class="w-full font-mono"
        :placeholder="$t('commands.linuxPh')"
      />
    </UFormField>
  </div>
</template>
