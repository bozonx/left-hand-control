<script setup lang="ts">
const props = withDefaults(defineProps<{
  text?: string
  disabled?: boolean
  align?: 'start' | 'center'
  ui?: Record<string, string>
}>(), {
  text: '',
  disabled: false,
  align: 'center',
  ui: () => ({}),
})

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
</script>

<template>
  <UTooltip :disabled="disabled" :ui="mergedUi" v-bind="$attrs">
    <slot />

    <template #content>
      <slot name="content">
        <div :class="textClass">{{ text }}</div>
      </slot>
    </template>
  </UTooltip>
</template>
