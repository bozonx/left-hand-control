export default defineAppConfig({
  ui: {
    colors: {
      primary: 'sky',
      neutral: 'zinc',
    },
    formField: {
      slots: {
        label: 'text-xs text-(--ui-text-muted)'
      }
    },
    modal: {
      slots: {
        overlay: 'fixed inset-0 bg-(--ui-bg)/80 transition-opacity'
      }
    }
  },
})
