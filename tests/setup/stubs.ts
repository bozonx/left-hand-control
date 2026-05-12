import { defineComponent, h } from 'vue'

export const AppTooltipStub = defineComponent({
  name: 'AppTooltip',
  props: ['text', 'disabled'],
  setup(_props, { slots }) {
    return () => slots.default?.()
  },
})

export const UModalStub = defineComponent({
  name: 'UModal',
  props: ['modelValue'],
  setup(props, { slots, emit }) {
    return () =>
      h('div', { 'data-testid': 'u-modal' }, [
        slots.default?.(),
        h('button', {
          'data-testid': 'u-modal-close',
          onClick: () => emit('update:modelValue', false),
        }),
      ])
  },
})

export const FieldResetButtonStub = defineComponent({
  name: 'FieldResetButton',
  props: ['label'],
  setup(_props, { emit }) {
    return () =>
      h('button', {
        'data-testid': 'field-reset-button',
        onClick: () => emit('click'),
      })
  },
})

export const UButtonStub = defineComponent({
  name: 'UButton',
  props: ['icon', 'color', 'variant'],
  setup(_props, { slots, attrs }) {
    return () => h('button', { ...attrs }, slots)
  },
})
