/* eslint-disable vue/one-component-per-file */
import { defineComponent, h } from 'vue'

export const AppTooltipStub = defineComponent({
  name: 'AppTooltip',
  props: { text: { type: String, default: '' }, disabled: { type: Boolean, default: false } },
  setup(_props, { slots }) {
    return () => slots.default?.()
  },
})

export const UModalStub = defineComponent({
  name: 'UModal',
  props: { modelValue: { type: [String, Number, Boolean], required: false, default: undefined } },
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
  props: { label: { type: String, required: false, default: '' } },
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
  props: { icon: { type: String, required: false, default: '' }, color: { type: String, required: false, default: '' }, variant: { type: String, required: false, default: '' } },
  setup(_props, { slots, attrs }) {
    return () => h('button', { ...attrs }, slots)
  },
})
