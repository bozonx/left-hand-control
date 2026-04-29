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
    const target = event.target as Node | null
    if (!target || !container.contains(target)) return

    if (event.key === 'Tab') {
      event.preventDefault()
      const ids = options.ids.value
      const idx = selectedId.value ? ids.indexOf(selectedId.value) : -1
      let next = event.shiftKey ? idx - 1 : idx + 1
      if (next < 0) next = ids.length - 1
      if (next >= ids.length) next = 0
      selectedId.value = ids[next] ?? null
      return
    }

    if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      if (!selectedId.value) return
      event.preventDefault()
      const delta = event.key === 'ArrowUp' ? -1 : 1
      options.move(selectedId.value, delta)
      return
    }
  }

  onMounted(() => document.addEventListener('keydown', onKeydown))
  onUnmounted(() => document.removeEventListener('keydown', onKeydown))

  return {
    selectedId,
    select,
    containerRef,
  }
}
