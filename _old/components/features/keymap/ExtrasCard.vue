<script setup lang="ts">
import RuleActionField from '~/components/features/rules/RuleActionField.vue'
import { VISUAL_KEY_CODES } from '~/utils/keys'

const props = defineProps<{
  extras: { key: string; action: string | null }[]
}>()

const emit = defineEmits<{
  add: [key: string]
  'clear-all': []
  'update-extra': [key: string, field: 'key' | 'action', value: string | null]
  remove: [key: string]
}>()

const addOpen = ref(false)
const newKey = ref<string | null>('')
const excludedKeys = computed(() => [...VISUAL_KEY_CODES, ...props.extras.map((extra) => extra.key)])
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
          <UButton icon="i-lucide-plus" size="sm" @click="newKey = ''; addOpen = true">
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
        v-for="extra in extras"
        :key="extra.key"
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
            single-key-only
            require-value
            :excluded-values="excludedKeys.filter((key) => key !== extra.key)"
            :placeholder="$t('rules.keyPh')"
            @update:model-value="(value: string | null) => emit('update-extra', extra.key, 'key', value ?? '')"
          />
        </UFormField>
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('keymap.extraActionLabel')"
              :hint="$t('keymap.extraActionHint')"
            />
          </template>
          <RuleActionField
            :model-value="extra.action"
            :placeholder="$t('rules.tapPh')"
            @update:model-value="(value: string | null) => emit('update-extra', extra.key, 'action', value)"
          />
        </UFormField>
        <div class="flex items-start gap-1">
          <UButton
            icon="i-lucide-trash-2"
            color="error"
            variant="ghost"
            size="sm"
            square
            :aria-label="$t('keymap.deleteExtra')"
            @click="$emit('remove', extra.key)"
          />
        </div>
      </div>
    </div>
    <ActionPickerModal
      v-model="newKey"
      v-model:open="addOpen"
      hide-trigger
      key-only
      single-key-only
      require-value
      :excluded-values="excludedKeys"
      @apply="(key: string) => emit('add', key)"
    />
  </UCard>
</template>
