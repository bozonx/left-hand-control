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
  ghost?: boolean
  ghostActive?: boolean
  ghostLabel?: string
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
  ghost: false,
  ghostActive: undefined,
  ghostLabel: undefined,
})

const emit = defineEmits<{
  'update:modelValue': [value: string | number | null | undefined]
}>()

const isReset = computed(() => props.modelValue === props.resetValue)
const showGhost = computed(() => props.ghost && (props.ghostActive ?? isReset.value))
const selectModel = computed({
  get: () => (
    props.emptyItemValue !== undefined && props.modelValue === props.emptyModelValue
      ? props.emptyItemValue
      : props.modelValue
  ),
  set: updateValue,
})

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
  <div class="flex w-full items-center gap-1">
    <USelectMenu
      v-model="selectModel"
      :items="props.items"
      :value-key="props.valueKey"
      :placeholder="props.placeholder"
      :search-input="props.searchable"
      class="w-full flex-1 min-w-0"
      :ui="showGhost ? {
        base: 'empty-field-dashed',
        value: 'truncate text-(--ui-text-muted)'
      } : undefined"
    >
      <template v-if="showGhost" #default>
        <span class="truncate text-(--ui-text-muted)">
          {{ props.ghostLabel ?? $t('common.notSet') }}
        </span>
      </template>
    </USelectMenu>
    <FieldResetButton
      v-if="props.clearable && !isReset"
      :label="props.resetAriaLabel || $t('common.reset')"
      @click="reset"
    />
  </div>
</template>
