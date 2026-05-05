<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import type { QuickAction } from '~/types/config'

defineProps<{
  isFirst?: boolean
  isLast?: boolean
}>()

const action = defineModel<QuickAction>('action', { required: true })

defineEmits<{
  remove: []
  moveUp: []
  moveDown: []
  pickAction: []
}>()
</script>

<template>
  <div
    class="group rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/30 p-3 transition-colors hover:border-(--ui-primary)/50 hover:bg-(--ui-bg-muted)/50"
  >
    <div class="flex flex-col gap-3 lg:flex-row lg:items-start">
      <div class="grid min-w-0 flex-1 grid-cols-1 gap-3 md:grid-cols-[minmax(0,1fr)_minmax(180px,240px)]">
        <UFormField>
          <template #label>
            <FieldLabel :label="$t('quickActions.nameLabel')" />
          </template>
          <UInput
            v-model="action.name"
            :placeholder="$t('quickActions.namePh')"
            class="w-full"
          />
        </UFormField>

        <UFormField class="min-w-0">
          <template #label>
            <FieldLabel :label="$t('quickActions.iconLabel')" />
          </template>
          <UInput
            v-model="action.icon"
            :placeholder="$t('quickActions.iconPh')"
            icon="i-lucide-image"
            class="w-full"
          />
        </UFormField>

        <UFormField class="md:col-span-2">
          <template #label>
            <FieldLabel :label="$t('quickActions.actionLabel')" />
          </template>
          <UButton
            color="neutral"
            variant="subtle"
            class="h-9 w-full justify-start overflow-hidden font-mono text-xs"
            :icon="action.action ? undefined : 'i-lucide-plus'"
            @click="$emit('pickAction')"
          >
            <span class="truncate">
              {{ action.action || $t('quickActions.actionPh') }}
            </span>
          </UButton>
        </UFormField>
      </div>

      <div class="flex shrink-0 justify-end gap-1 lg:pt-6">
        <AppTooltip :text="$t('common.moveUp')">
          <UButton
            icon="i-lucide-arrow-up"
            variant="ghost"
            color="neutral"
            size="sm"
            square
            :disabled="isFirst"
            :aria-label="$t('quickActions.moveUp')"
            @click="$emit('moveUp')"
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
            :aria-label="$t('quickActions.moveDown')"
            @click="$emit('moveDown')"
          />
        </AppTooltip>
        <AppTooltip :text="$t('quickActions.deleteAction')">
          <UButton
            icon="i-lucide-trash-2"
            variant="ghost"
            color="error"
            size="sm"
            square
            :aria-label="$t('quickActions.deleteAction')"
            @click="$emit('remove')"
          />
        </AppTooltip>
      </div>
    </div>
  </div>
</template>
