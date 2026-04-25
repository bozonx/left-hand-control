<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { LayerRule } from '~/types/config'
import { useLayout } from '~/composables/useLayout'

const props = defineProps<{
  rule: LayerRule
}>()

const isOpen = defineModel<boolean>('open', { default: false })

const { t } = useI18n()
const { systemLayouts } = useLayout()

const gameModeOptions = computed(() => [
  { label: t('rules.gameModeIgnore'), value: 'ignore' },
  { label: t('rules.gameModeOn'), value: 'on' },
  { label: t('rules.gameModeOff'), value: 'off' },
])

const layoutOptions = computed(() => {
  return systemLayouts.value.map(layout => ({
    label: layout.display ? `${layout.short} (${layout.display})` : layout.short,
    value: layout.short
  }))
})

// Initialize defaults if undefined
if (!props.rule.conditionGameMode) {
  props.rule.conditionGameMode = 'ignore'
}
if (!props.rule.conditionLayouts) {
  props.rule.conditionLayouts = []
}



</script>

<template>
  <UModal v-model:open="isOpen" :title="$t('rules.conditionsLabel')">
    <template #body>
      <div class="flex flex-col gap-6">
        <UFormField :label="$t('rules.gameModeLabel')" :help="$t('rules.gameModeHint')">
          <USelectMenu
            v-model="rule.conditionGameMode"
            :items="gameModeOptions"
            value-key="value"
          />
        </UFormField>

        <UFormField :label="$t('rules.layoutsLabel')" :help="$t('rules.layoutsHint')">
          <div class="flex flex-col gap-2 p-3 border rounded-lg border-(--ui-border) max-h-60 overflow-y-auto bg-(--ui-bg-muted)/30">
            <template v-if="layoutOptions.length > 0">
              <UCheckbox
                v-for="option in layoutOptions"
                :key="option.value"
                :label="option.label"
                :model-value="rule.conditionLayouts?.includes(option.value)"
                @update:model-value="(checked: boolean) => {
                  const current = rule.conditionLayouts || []
                  if (checked) {
                    rule.conditionLayouts = [...current, option.value]
                  } else {
                    rule.conditionLayouts = current.filter(v => v !== option.value)
                  }
                }"
              />
            </template>
            <div v-else class="text-sm text-(--ui-text-muted) italic">
              {{ $t('rules.noLayoutsDetected') }}
            </div>
          </div>
        </UFormField>
      </div>
    </template>

    <template #footer>
      <div class="flex justify-end w-full">
        <UButton @click="isOpen = false">{{ $t('common.close') }}</UButton>
      </div>
    </template>
  </UModal>
</template>
