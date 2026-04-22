<script setup lang="ts">
const props = withDefaults(defineProps<{
  items: Array<Record<string, any>>
  modelValue?: string | number | null
  placeholder?: string
  valueKey?: string
  resetValue?: string | number | null
  resetAriaLabel?: string
  clearable?: boolean
}>(), {
  modelValue: undefined,
  placeholder: '',
  valueKey: 'value',
  resetValue: undefined,
  resetAriaLabel: '',
  clearable: true,
})

const emit = defineEmits<{
  'update:modelValue': [value: string | number | null | undefined]
}>()

const isReset = computed(() => props.modelValue === props.resetValue)

function reset() {
  emit('update:modelValue', props.resetValue)
}

function updateValue(value: string | number | null | undefined) {
  emit('update:modelValue', value)
}
</script>

<template>
  <div class="flex items-center gap-1">
    <USelectMenu
      :model-value="props.modelValue"
      :items="props.items"
      :value-key="props.valueKey"
      :placeholder="props.placeholder"
      class="flex-1 min-w-0"
      @update:model-value="updateValue"
    />
    <UButton
      v-if="props.clearable && !isReset"
      icon="i-lucide-rotate-ccw"
      variant="ghost"
      color="neutral"
      square
      :aria-label="props.resetAriaLabel || $t('common.reset')"
      @click="reset"
    />
  </div>
</template>
