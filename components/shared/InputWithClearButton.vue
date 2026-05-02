<script setup lang="ts">
const props = withDefaults(defineProps<{
  modelValue?: string
  placeholder?: string
  disabled?: boolean
}>(), {
  modelValue: '',
  placeholder: undefined,
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
  clear: []
  focus: [event: FocusEvent]
  blur: [event: FocusEvent]
  keydown: [event: KeyboardEvent]
}>()

const model = computed({
  get: () => props.modelValue,
  set: (v: string) => emit('update:modelValue', v),
})

const hasValue = computed(() => props.modelValue.length > 0)

function onClear() {
  model.value = ''
  emit('clear')
}

const wrapperRef = ref<HTMLDivElement | null>(null)

function focus() {
  const input = wrapperRef.value?.querySelector('input')
  input?.focus()
}

defineExpose({ focus })
</script>

<template>
  <div ref="wrapperRef">
    <UInput
      v-model="model"
      :placeholder="props.placeholder"
      :disabled="props.disabled"
      class="w-full"
      :ui="{ trailing: 'pe-1' }"
      @focus="(e: FocusEvent) => $emit('focus', e)"
      @blur="(e: FocusEvent) => $emit('blur', e)"
      @keydown="(e: KeyboardEvent) => $emit('keydown', e)"
    >
    <template #trailing>
      <div class="flex items-center gap-0.5">
        <UButton
          v-if="hasValue"
          icon="i-lucide-x"
          variant="link"
          color="neutral"
          size="sm"
          :aria-label="$t('common.clear')"
          @mousedown.stop.prevent
          @click="onClear"
        />
        <slot name="extra-trailing" />
      </div>
    </template>
    </UInput>
  </div>
</template>
