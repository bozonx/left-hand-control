<script setup lang="ts">
import type { ComponentPublicInstance } from 'vue'

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

function onWheel(event: WheelEvent) {
  const target = event.target as HTMLInputElement | null
  if (!target || document.activeElement !== target) return

  event.preventDefault()

  const delta = event.deltaY > 0 ? -1 : 1
  const step = event.shiftKey ? props.step * 10 : props.step
  const current = parseValue()
  if (current === null) return

  clampAndEmit(current + delta * step)
}

const inputRef = ref<(ComponentPublicInstance & { $el: HTMLElement }) | null>(null)

function focus() {
  const el = inputRef.value?.$el.querySelector('input')
  el?.focus()
}

function select() {
  const el = inputRef.value?.$el.querySelector('input')
  el?.select()
}

defineExpose({ focus, select })
</script>

<template>
  <UInput
    ref="inputRef"
    :model-value="String(modelValue)"
    type="number"
    :min="min"
    :max="max"
    :step="step"
    :size="size"
    :class="class"
    @update:model-value="(v: string | number) => emit('update:modelValue', v)"
    @blur="(e: FocusEvent) => emit('blur', e)"
    @focus="(e: FocusEvent) => emit('focus', e)"
    @keydown="(e: KeyboardEvent) => emit('keydown', e)"
    @wheel="onWheel"
  />
</template>
