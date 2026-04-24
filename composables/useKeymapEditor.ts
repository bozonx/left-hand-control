import { BASE_LAYER_ID } from '~/types/config'
import { randomId } from '~/utils/keys'

export function useKeymapEditor() {
  const { config } = useConfig()
  const uiState = useUiState()
  const {
    ensureLayerKeymap,
    createLayer,
    renameLayer,
    deleteLayer,
  } = useLayers()

  const selectedLayerId = computed<string>({
    get: () => uiState.state.value.selectedLayerId || BASE_LAYER_ID,
    set: (value) => {
      uiState.setSelectedLayerId(value)
    },
  })

  watch(
    () => config.value.layers.map((layer) => layer.id).join(','),
    () => {
      if (!config.value.layers.some((layer) => layer.id === selectedLayerId.value)) {
        uiState.setSelectedLayerId(BASE_LAYER_ID)
      }
    },
    { immediate: true },
  )

  const layerItems = computed(() =>
    config.value.layers.map((layer) => ({ label: layer.name, value: layer.id })),
  )

  const currentLayer = computed(() =>
    config.value.layers.find((layer) => layer.id === selectedLayerId.value),
  )

  const currentKeymap = computed(() => ensureLayerKeymap(selectedLayerId.value))

  const editOpen = ref(false)
  const editKeyCode = ref('')
  const editKeyLabel = ref('')
  const editAction = ref('')

  function openEdit(code: string, label: string) {
    editKeyCode.value = code
    editKeyLabel.value = label
    editAction.value = currentKeymap.value.keys[code] ?? ''
    editOpen.value = true
  }

  function saveEdit(action: string) {
    if (action) currentKeymap.value.keys[editKeyCode.value] = action
    else delete currentKeymap.value.keys[editKeyCode.value]
  }

  function clearEdit() {
    delete currentKeymap.value.keys[editKeyCode.value]
  }

  function addExtra() {
    currentKeymap.value.extras.unshift({
      id: randomId(),
      name: '',
      action: '',
    })
  }

  function moveExtra(id: string, direction: 'up' | 'down') {
    const index = currentKeymap.value.extras.findIndex((extra) => extra.id === id)
    if (index === -1) return
    const newIndex = direction === 'up' ? index - 1 : index + 1
    if (newIndex < 0 || newIndex >= currentKeymap.value.extras.length) return
    const [extra] = currentKeymap.value.extras.splice(index, 1)
    if (!extra) return
    currentKeymap.value.extras.splice(newIndex, 0, extra)
  }

  function removeExtra(id: string) {
    currentKeymap.value.extras = currentKeymap.value.extras.filter(
      (extra) => extra.id !== id,
    )
  }

  const renameOpen = ref(false)
  const renameDraftName = ref('')
  const deleteConfirmOpen = ref(false)

  function openRename() {
    const layer = currentLayer.value
    renameDraftName.value = layer?.name ?? ''
    renameOpen.value = true
  }

  function confirmRename() {
    const layerId = currentLayer.value?.id
    if (layerId) {
      renameLayer(layerId, {
        name: renameDraftName.value,
        description: currentLayer.value?.description,
      })
    }
    renameOpen.value = false
  }

  function updateCurrentLayerDescription(description: string) {
    const layer = currentLayer.value
    if (!layer) return false
    return renameLayer(layer.id, {
      name: layer.name,
      description,
    })
  }

  function deleteSelectedLayer() {
    if (deleteLayer(selectedLayerId.value)) {
      selectedLayerId.value = BASE_LAYER_ID
      deleteConfirmOpen.value = false
    }
  }

  function requestDeleteSelectedLayer() {
    if (selectedLayerId.value === BASE_LAYER_ID) return
    deleteConfirmOpen.value = true
  }

  function cancelDeleteSelectedLayer() {
    deleteConfirmOpen.value = false
  }

  const newLayerOpen = ref(false)
  const newLayerName = ref('')

  function openNewLayer() {
    newLayerName.value = ''
    newLayerOpen.value = true
  }

  function confirmNewLayer() {
    const id = createLayer({
      name: newLayerName.value,
    })
    if (!id) return
    selectedLayerId.value = id
    newLayerOpen.value = false
  }

  return {
    config,
    selectedLayerId,
    layerItems,
    currentLayer,
    currentKeymap,
    editOpen,
    editKeyCode,
    editKeyLabel,
    editAction,
    openEdit,
    saveEdit,
    clearEdit,
    addExtra,
    moveExtra,
    removeExtra,
    renameOpen,
    renameDraftName,
    openRename,
    confirmRename,
    updateCurrentLayerDescription,
    deleteConfirmOpen,
    requestDeleteSelectedLayer,
    cancelDeleteSelectedLayer,
    deleteSelectedLayer,
    newLayerOpen,
    newLayerName,
    openNewLayer,
    confirmNewLayer,
  }
}
