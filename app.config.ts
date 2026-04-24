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
      },
      variants: {
        transition: {
          true: {
            overlay: 'data-[state=open]:animate-[fade-in_200ms_ease-out] data-[state=closed]:animate-[fade-out_200ms_ease-in]',
            content: ''
          }
        }
      },
      compoundVariants: [
        {
          scrollable: false,
          fullscreen: false,
          class: {
            content: '!top-0 !right-0 !bottom-0 !left-0 !m-auto !translate-x-0 !translate-y-0 w-[calc(100vw-2rem)] max-w-lg max-h-[calc(100dvh-2rem)] sm:max-h-[calc(100dvh-4rem)] h-fit overflow-hidden'
          }
        }
      ]
    },
    tooltip: {
      slots: {
        content: 'flex items-center gap-1 bg-default text-highlighted shadow-sm rounded-sm ring ring-default h-6 px-2.5 py-1 text-xs select-none pointer-events-auto'
      }
    },
    popover: {
      slots: {
        content: 'bg-default shadow-lg rounded-md ring ring-default focus:outline-none pointer-events-auto'
      }
    },
    select: {
      slots: {
        content: 'max-h-60 w-(--reka-select-trigger-width) bg-default shadow-lg rounded-md ring ring-default overflow-hidden pointer-events-auto flex flex-col'
      }
    },
    selectMenu: {
      slots: {
        content: 'max-h-60 w-(--reka-select-trigger-width) bg-default shadow-lg rounded-md ring ring-default overflow-hidden pointer-events-auto flex flex-col origin-(--reka-combobox-content-transform-origin) w-(--reka-combobox-trigger-width)'
      }
    },
    inputMenu: {
      slots: {
        content: 'max-h-60 w-(--reka-combobox-trigger-width) bg-default shadow-lg rounded-md ring ring-default overflow-hidden pointer-events-auto flex flex-col'
      }
    },
    dropdownMenu: {
      slots: {
        content: 'min-w-32 bg-default shadow-lg rounded-md ring ring-default overflow-hidden flex flex-col'
      }
    },
    contextMenu: {
      slots: {
        content: 'min-w-32 bg-default shadow-lg rounded-md ring ring-default overflow-hidden flex flex-col'
      }
    }
  },
})
