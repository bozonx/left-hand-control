<script setup lang="ts">
import type { ExtraKey } from '~/types/config'

defineProps<{
  extras: ExtraKey[]
}>()

defineEmits<{
  add: []
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
        <UButton icon="i-lucide-plus" size="sm" @click="$emit('add')">
          {{ $t('keymap.addExtra') }}
        </UButton>
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
        class="grid grid-cols-[minmax(12rem,0.9fr)_minmax(14rem,1.1fr)_auto] gap-3 items-start p-3 rounded-md border border-(--ui-border) bg-(--ui-bg-muted) transition-all duration-200 hover:border-(--ui-primary)/50 hover:bg-(--ui-bg-elevated) hover:shadow-md"
      >
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('keymap.extraKeyLabel')"
              :hint="$t('keymap.extraKeyHint')"
            />
          </template>
          <ActionPickerModal
            v-model="extra.key"
            key-only
            :placeholder="$t('rules.keyPh')"
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
            v-model="extra.action"
            :placeholder="$t('rules.tapPh')"
          />
        </UFormField>
        <div class="flex items-start gap-1 pt-6">
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
