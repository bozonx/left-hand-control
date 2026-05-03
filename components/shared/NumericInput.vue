<script setup lang="ts">
const props = withDefaults(defineProps<{
  modelValue?: string | number
  min?: number
  max?: number
  step?: number
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl'
  class?: string
}>(), {
  modelValue: '',
  min: undefined,
  max: undefined,
  step: 1,
  size: 'md',
  class: '',
})

const emit = defineEmits<{
  'update:modelValue': [value: string | number]
  blur: [event: FocusEvent]
  focus: [event: FocusEvent]
  keydown: [event: KeyboardEvent]
}>()

const rootClass = computed(() => props.class)

function parseValue(): number | null {
  const v = typeof props.modelValue === 'number' ? props.modelValue : Number(props.modelValue)
  if (!Number.isFinite(v)) return null
  return v
}

function clampAndEmit(raw: number) {
  let next = raw
  if (props.min !== undefined) next = Math.max(props.min, next)
  if (props.max !== undefined) next = Math.min(props.max, next)
  emit('update:modelValue', next)
}

const rootRef = ref<HTMLElement | null>(null)
let wheelInput: HTMLInputElement | null = null

function getInput() {
  return rootRef.value?.querySelector('input') ?? null
}

function focus() {
  getInput()?.focus()
}

function select() {
  getInput()?.select()
}

defineExpose({ focus, select })

function onWheel(event: WheelEvent) {
  const input = getInput()
  if (!input || document.activeElement !== input) return

  event.preventDefault()

  const delta = event.deltaY > 0 ? -1 : 1
  const step = event.shiftKey ? props.step * 10 : props.step
  const current = parseValue()
  if (current === null) return

  clampAndEmit(current + delta * step)
}

onMounted(() => {
  nextTick(() => {
    wheelInput = getInput() ?? null
    wheelInput?.addEventListener('wheel', onWheel, { passive: false })
  })
})

onBeforeUnmount(() => {
  wheelInput?.removeEventListener('wheel', onWheel)
})
</script>

<template>
  <span ref="rootRef" class="inline-block" :class="rootClass">
    <UInput
      :model-value="String(modelValue)"
      type="number"
      :min="min"
      :max="max"
      :step="step"
      :size="size"
      class="w-full"
      @update:model-value="(v: string | number) => emit('update:modelValue', v)"
      @blur="(e: FocusEvent) => emit('blur', e)"
      @focus="(e: FocusEvent) => emit('focus', e)"
      @keydown="(e: KeyboardEvent) => emit('keydown', e)"
    />
  </span>
</template>
