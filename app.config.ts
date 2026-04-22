export default defineAppConfig({
  ui: {
    colors: {
      primary: 'sky',
      neutral: 'zinc',
    },
    formField: {
      slots: {
        label: 'text-(--ui-text-muted)'
      }
    }
  },
})
