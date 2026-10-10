import type { Layer, LayerKeymap } from '~/types/config'

export function useKeymapKeyEdit(
  currentKeymap: ComputedRef<LayerKeymap>,
  currentLayer: ComputedRef<Layer | undefined>,
) {
  const editOpen = ref(false)
  const editKeyCode = ref('')
  const editAction = ref<string | null | undefined>('')

  function openEdit(code: string) {
    editKeyCode.value = code
    editAction.value = currentKeymap.value.keys[code]
    editOpen.value = true
  }

  function saveEdit(action: string) {
    if (!currentLayer.value) return
    if (action) currentKeymap.value.keys[editKeyCode.value] = action
    else delete currentKeymap.value.keys[editKeyCode.value]
  }

  function clearEdit() {
    if (!currentLayer.value) return
    delete currentKeymap.value.keys[editKeyCode.value]
  }

  function swallowEdit() {
    if (!currentLayer.value) return
    currentKeymap.value.keys[editKeyCode.value] = null
  }

  return {
    editOpen,
    editKeyCode,
    editAction,
    openEdit,
    saveEdit,
    clearEdit,
    swallowEdit,
  }
}
