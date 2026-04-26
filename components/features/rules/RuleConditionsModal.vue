<script setup lang="ts">
import { computed } from 'vue'
import type { LayerRule } from '~/types/config'
import ConditionsForm, { type ConditionsValue } from '~/components/features/shared/ConditionsForm.vue'

const props = defineProps<{
  rule: LayerRule
}>()

const isOpen = defineModel<boolean>('open', { default: false })

// Initialize defaults if undefined
if (!props.rule.conditionGameMode) {
  props.rule.conditionGameMode = 'ignore'
}
if (!props.rule.conditionLayouts) {
  props.rule.conditionLayouts = []
}

const conditions = computed<ConditionsValue>({
  get: () => ({
    gameMode: props.rule.conditionGameMode ?? 'ignore',
    layouts: props.rule.conditionLayouts ?? [],
  }),
  set: (value) => {
    props.rule.conditionGameMode = value.gameMode
    props.rule.conditionLayouts = value.layouts
  },
})
</script>

<template>
  <UModal v-model:open="isOpen" :title="$t('rules.conditionsLabel')">
    <template #body>
      <ConditionsForm v-model="conditions" />
    </template>

    <template #footer>
      <div class="flex justify-end w-full">
        <UButton @click="isOpen = false">{{ $t('common.close') }}</UButton>
      </div>
    </template>
  </UModal>
</template>
