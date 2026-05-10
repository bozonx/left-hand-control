<script setup lang="ts">
import type { ExtraKey } from '~/types/config'
import { VISUAL_KEY_CODES } from '~/utils/keys'

defineProps<{
  extras: ExtraKey[]
}>()

const emit = defineEmits<{
  add: []
  'clear-all': []
  'update-extra': [id: string, field: 'key' | 'action', value: string]
  moveUp: [id: string]
  moveDown: [id: string]
  remove: [id: string]
}>()

</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-sm font-semibold">{{ $t('keymap.extrasTitle') }}</h2>
          <p class="text-xs text-(--ui-text-muted) mt-0.5">
            {{ $t('keymap.extrasSub') }}
          </p>
        </div>
        <div class="flex items-center gap-2">
          <UButton
            v-if="extras.length"
            icon="i-lucide-eraser"
            size="sm"
            color="neutral"
            variant="ghost"
            @click="$emit('clear-all')"
          >
            {{ $t('common.clear') }}
          </UButton>
          <UButton icon="i-lucide-plus" size="sm" @click="$emit('add')">
            {{ $t('keymap.addExtra') }}
          </UButton>
        </div>
      </div>
    </template>
    <div
      v-if="extras.length === 0"
      class="text-sm text-(--ui-text-muted)"
    >
      {{ $t('keymap.extrasEmpty') }}
    </div>
    <div v-else class="space-y-2">
      <div
        v-for="(extra, index) in extras"
        :key="extra.id"
        class="grid grid-cols-[minmax(12rem,0.9fr)_minmax(14rem,1.1fr)_auto] items-center gap-3 rounded-md border border-(--ui-border) bg-(--ui-bg-muted) p-3 transition-all duration-200 hover:border-(--ui-primary)/50 hover:bg-(--ui-bg-elevated) hover:shadow-md"
      >
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('keymap.extraKeyLabel')"
              :hint="$t('keymap.extraKeyHint')"
            />
          </template>
          <ActionPickerModal
            :model-value="extra.key"
            key-only
            :excluded-values="VISUAL_KEY_CODES"
            :excluded-category-ids="['lettersSymbols']"
            :placeholder="$t('rules.keyPh')"
            @update:model-value="(value: string | null) => emit('update-extra', extra.id, 'key', value ?? '')"
          />
        </UFormField>
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('keymap.extraActionLabel')"
              :hint="$t('keymap.extraActionHint')"
            />
          </template>
          <ActionPickerModal
            :model-value="extra.action"
            :placeholder="$t('rules.tapPh')"
            @update:model-value="(value: string | null) => emit('update-extra', extra.id, 'action', value ?? '')"
          />
        </UFormField>
        <div class="flex items-start gap-1">
          <UButton
            icon="i-lucide-arrow-up"
            variant="ghost"
            color="neutral"
            size="sm"
            square
            :disabled="index === 0"
            :aria-label="$t('keymap.moveExtraUp')"
            @click="$emit('moveUp', extra.id)"
          />
          <UButton
            icon="i-lucide-arrow-down"
            variant="ghost"
            color="neutral"
            size="sm"
            square
            :disabled="index === extras.length - 1"
            :aria-label="$t('keymap.moveExtraDown')"
            @click="$emit('moveDown', extra.id)"
          />
          <UButton
            icon="i-lucide-trash-2"
            color="error"
            variant="ghost"
            size="sm"
            square
            :aria-label="$t('keymap.deleteExtra')"
            @click="$emit('remove', extra.id)"
          />
        </div>
      </div>
    </div>
  </UCard>
</template>
