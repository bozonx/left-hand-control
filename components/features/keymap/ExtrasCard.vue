<script setup lang="ts">
import type { ExtraKey } from '~/types/config'

defineProps<{
  extras: ExtraKey[]
}>()

defineEmits<{
  add: []
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
        v-for="extra in extras"
        :key="extra.id"
        class="grid grid-cols-[minmax(12rem,0.9fr)_minmax(14rem,1.1fr)_auto] gap-3 items-start p-3 rounded-md border border-(--ui-border) bg-(--ui-bg-muted)"
      >
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('keymap.extraKeyLabel')"
              :hint="$t('keymap.extraKeyHint')"
            />
          </template>
          <ActionPickerModal
            v-model="extra.name"
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
            allow-empty
            :placeholder="$t('rules.tapPh')"
          />
        </UFormField>
        <div class="pt-6">
          <UButton
            icon="i-lucide-trash-2"
            color="error"
            variant="ghost"
            square
            :aria-label="$t('keymap.deleteExtra')"
            @click="$emit('remove', extra.id)"
          />
        </div>
      </div>
    </div>
  </UCard>
</template>
