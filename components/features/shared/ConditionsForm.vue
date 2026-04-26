<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useLayout } from '~/composables/useLayout'

// Canonical shape used by both per-rule conditions and layout
// whitelist / blacklist conditions. `gameMode === 'ignore'` represents
// "do not check Game Mode"; an empty `layouts` array represents
// "do not check the system keyboard layout".
export interface ConditionsValue {
  gameMode: 'ignore' | 'on' | 'off'
  layouts: string[]
}

const props = defineProps<{
  modelValue: ConditionsValue
}>()

const emit = defineEmits<{
  'update:modelValue': [value: ConditionsValue]
}>()

const { t } = useI18n()
const { systemLayouts } = useLayout()

const gameModeOptions = computed(() => [
  { label: t('rules.gameModeIgnore'), value: 'ignore' as const },
  { label: t('rules.gameModeOn'), value: 'on' as const },
  { label: t('rules.gameModeOff'), value: 'off' as const },
])

const layoutOptions = computed(() =>
  systemLayouts.value.map((layout) => ({
    label: layout.display ? `${layout.short} (${layout.display})` : layout.short,
    value: layout.short,
  })),
)

const gameMode = computed({
  get: () => props.modelValue.gameMode,
  set: (value) => emit('update:modelValue', { ...props.modelValue, gameMode: value }),
})

function toggleLayout(value: string, checked: boolean | 'indeterminate') {
  const current = props.modelValue.layouts
  const next = checked === true
    ? [...current, value]
    : current.filter((v) => v !== value)
  emit('update:modelValue', { ...props.modelValue, layouts: next })
}
</script>

<template>
  <div class="flex flex-col gap-6">
    <UFormField :label="$t('rules.gameModeLabel')" :help="$t('rules.gameModeHint')">
      <USelectMenu v-model="gameMode" :items="gameModeOptions" value-key="value" />
    </UFormField>

    <UFormField :label="$t('rules.layoutsLabel')" :help="$t('rules.layoutsHint')">
      <div
        class="flex flex-col gap-2 p-3 border rounded-lg border-(--ui-border) max-h-60 overflow-y-auto bg-(--ui-bg-muted)/30"
      >
        <template v-if="layoutOptions.length > 0">
          <UCheckbox
            v-for="option in layoutOptions"
            :key="option.value"
            :label="option.label"
            :model-value="modelValue.layouts.includes(option.value)"
            @update:model-value="(checked) => toggleLayout(option.value, checked)"
          />
        </template>
        <div v-else class="text-sm text-(--ui-text-muted) italic">
          {{ $t('rules.noLayoutsDetected') }}
        </div>
      </div>
    </UFormField>
  </div>
</template>
