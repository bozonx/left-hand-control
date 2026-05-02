<script setup lang="ts">
import FieldResetButton from '~/components/shared/FieldResetButton.vue'

const props = withDefaults(defineProps<{
  items: Array<Record<string, unknown>>
  modelValue?: string | number | null
  placeholder?: string
  valueKey?: string
  resetValue?: string | number | null
  resetAriaLabel?: string
  clearable?: boolean
  searchable?: boolean
  emptyItemValue?: string | number | null
  emptyModelValue?: string | number | null
}>(), {
  modelValue: undefined,
  placeholder: '',
  valueKey: 'value',
  resetValue: undefined,
  resetAriaLabel: '',
  clearable: true,
  searchable: true,
  emptyItemValue: undefined,
  emptyModelValue: undefined,
})

const emit = defineEmits<{
  'update:modelValue': [value: string | number | null | undefined]
}>()

const isReset = computed(() => props.modelValue === props.resetValue)

function reset() {
  emit('update:modelValue', props.resetValue)
}

function updateValue(value: string | number | null | undefined) {
  emit(
    'update:modelValue',
    props.emptyItemValue !== undefined && value === props.emptyItemValue
      ? props.emptyModelValue
      : value,
  )
}
</script>

<template>
  <div class="flex items-center gap-1">
    <USelectMenu
      :model-value="props.modelValue"
      :items="props.items"
      :value-key="props.valueKey"
      :placeholder="props.placeholder"
      :search-input="props.searchable"
      class="flex-1 min-w-0"
      @update:model-value="updateValue"
    />
    <FieldResetButton
      v-if="props.clearable && !isReset"
      :label="props.resetAriaLabel || $t('common.reset')"
      @click="reset"
    />
  </div>
</template>
