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
})

const emit = defineEmits<{
  'update:modelValue': [value: string | number | null | undefined]
}>()

const isReset = computed(() => props.modelValue === props.resetValue)
const showGhost = computed(() => props.ghost && isReset.value)
const ghostOpen = ref(false)

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
    <template v-if="showGhost && !ghostOpen">
      <UButton
        variant="ghost"
        color="neutral"
        class="flex-1 min-w-0 h-8 px-2.5 justify-start border border-dashed border-(--ui-border) text-(--ui-text-muted) hover:text-(--ui-text) hover:border-(--ui-border-accent) hover:bg-(--ui-bg-elevated)/50"
        @click="ghostOpen = true"
      >
        <UIcon name="i-lucide-plus" class="shrink-0 w-4 h-4 mr-1.5" />
        <span class="truncate">{{ props.placeholder }}</span>
      </UButton>
    </template>
    <template v-else>
      <USelectMenu
        :model-value="props.modelValue"
        :items="props.items"
        :value-key="props.valueKey"
        :placeholder="props.placeholder"
        :search-input="props.searchable"
        class="flex-1 min-w-0"
        @update:model-value="(v: unknown) => updateValue(v as string | number | null | undefined)"
      />
    </template>
    <FieldResetButton
      v-if="props.clearable && !isReset"
      :label="props.resetAriaLabel || $t('common.reset')"
      @click="reset"
    />
  </div>
</template>
