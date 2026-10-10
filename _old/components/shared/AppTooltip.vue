<script setup lang="ts">
defineOptions({
  inheritAttrs: false,
})

const props = withDefaults(defineProps<{
  text?: string
  disabled?: boolean
  align?: 'start' | 'center'
  ui?: Record<string, string>
  hoverDelay?: number
  toggleOnClick?: boolean
}>(), {
  text: '',
  disabled: false,
  align: 'center',
  ui: () => ({}),
  hoverDelay: 500,
  toggleOnClick: false,
})

const attrs = useAttrs()
const triggerRef = ref<HTMLElement | null>(null)
const hoverOpen = ref(false)
const pinnedOpen = ref(false)
const suppressHoverUntilLeave = ref(false)
let hoverTimer: ReturnType<typeof setTimeout> | null = null

const mergedUi = computed(() => ({
  ...props.ui,
  content: [
    'h-auto max-w-72 py-2',
    props.align === 'start' ? 'items-start' : '',
    props.ui.content ?? '',
  ].filter(Boolean).join(' '),
}))

const textClass = computed(() =>
  [
    'whitespace-pre-wrap',
    props.align === 'start' ? 'text-left' : 'text-center',
  ].join(' '),
)

const isOpen = computed(() =>
  !props.disabled && (hoverOpen.value || pinnedOpen.value),
)

function clearHoverTimer() {
  if (!hoverTimer) return
  clearTimeout(hoverTimer)
  hoverTimer = null
}

function showFromHover() {
  clearHoverTimer()
  if (props.disabled || suppressHoverUntilLeave.value) return
  hoverTimer = setTimeout(() => {
    hoverOpen.value = true
    hoverTimer = null
  }, props.hoverDelay)
}

function onMouseEnter() {
  if (props.disabled || suppressHoverUntilLeave.value) return
  showFromHover()
}

function onMouseLeave() {
  clearHoverTimer()
  hoverOpen.value = false
  suppressHoverUntilLeave.value = false
}

function onClick() {
  if (props.disabled || !props.toggleOnClick) return

  if (hoverTimer) {
    clearHoverTimer()
    hoverOpen.value = true
  }

  if (isOpen.value) {
    pinnedOpen.value = false
    hoverOpen.value = false
    suppressHoverUntilLeave.value = true
    return
  }

  suppressHoverUntilLeave.value = false
  pinnedOpen.value = true
}

watch(() => props.disabled, (disabled) => {
  if (!disabled) return
  clearHoverTimer()
  hoverOpen.value = false
  pinnedOpen.value = false
  suppressHoverUntilLeave.value = false
})

onBeforeUnmount(() => {
  clearHoverTimer()
})
</script>

<template>
  <UTooltip
    :open="isOpen"
    :reference="triggerRef"
    :disabled="disabled"
    :delay-duration="0"
    :ui="mergedUi"
  >
      <span
        ref="triggerRef"
        v-bind="attrs"
        class="inline-flex max-w-full"
        @mouseenter="onMouseEnter"
        @mouseleave="onMouseLeave"
        @click="onClick"
      >
        <slot />
      </span>

    <template #content>
      <slot name="content">
        <div :class="textClass">{{ text }}</div>
      </slot>
    </template>
  </UTooltip>
</template>
