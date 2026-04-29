export interface ListKeyboardNavigationOptions {
  ids: Ref<string[]>
  move: (id: string, delta: number) => void
}

export function useListKeyboardNavigation(options: ListKeyboardNavigationOptions) {
  const selectedId = ref<string | null>(null)
  const containerRef = useTemplateRef<HTMLElement>('containerRef')

  function select(id: string | null) {
    selectedId.value = id
  }

  function onKeydown(event: KeyboardEvent) {
    const container = containerRef.value
    if (!container) return
    const target = event.target as HTMLElement | null
    if (!target || !container.contains(target)) return

    // ignore when focus is inside interactive controls (inputs, pickers, etc.)
    if (target.closest('input, textarea, select, button, [role="dialog"], [role="listbox"]')) {
      if (event.key === 'Escape' && selectedId.value) {
        event.preventDefault()
        selectedId.value = null
      }
      return
    }

    if (event.key === 'Tab') {
      if (!selectedId.value) return
      event.preventDefault()
      const ids = options.ids.value
      const idx = ids.indexOf(selectedId.value)
      let next = event.shiftKey ? idx - 1 : idx + 1
      if (next < 0) next = ids.length - 1
      if (next >= ids.length) next = 0
      selectedId.value = ids[next] ?? null
      return
    }

    if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      if (!selectedId.value) return
      event.preventDefault()
      event.stopPropagation()
      const delta = event.key === 'ArrowUp' ? -1 : 1
      options.move(selectedId.value, delta)
      return
    }

    if (event.key === 'Escape') {
      if (!selectedId.value) return
      event.preventDefault()
      selectedId.value = null
      return
    }
  }

  function onDocumentClick(event: MouseEvent) {
    const container = containerRef.value
    if (!container) return
    const target = event.target as Node | null
    if (!target || !container.contains(target)) {
      selectedId.value = null
    }
  }

  onMounted(() => {
    document.addEventListener('keydown', onKeydown)
    document.addEventListener('click', onDocumentClick)
  })
  onUnmounted(() => {
    document.removeEventListener('keydown', onKeydown)
    document.removeEventListener('click', onDocumentClick)
  })

  return {
    selectedId,
    select,
    containerRef,
  }
}
