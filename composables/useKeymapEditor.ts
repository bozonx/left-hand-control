import { BASE_LAYER_ID } from '~/types/config'
import { randomId } from '~/utils/keys'

export function useKeymapEditor() {
  const { config } = useConfig()
  const {
    ensureLayerKeymap,
    createLayer,
    renameLayer,
    deleteLayer,
  } = useLayers()

  const selectedLayerId = ref<string>(BASE_LAYER_ID)

  watch(
    () => config.value.layers.map((layer) => layer.id).join(','),
    () => {
      if (!config.value.layers.some((layer) => layer.id === selectedLayerId.value)) {
        selectedLayerId.value = BASE_LAYER_ID
      }
    },
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
    currentKeymap.value.extras.push({
      id: randomId(),
      name: '',
      action: '',
    })
  }

  function removeExtra(id: string) {
    currentKeymap.value.extras = currentKeymap.value.extras.filter(
      (extra) => extra.id !== id,
    )
  }

  const renameOpen = ref(false)
  const renameDraftName = ref('')
  const renameDraftDescription = ref('')

  function openRename() {
    const layer = currentLayer.value
    renameDraftName.value = layer?.name ?? ''
    renameDraftDescription.value = layer?.description ?? ''
    renameOpen.value = true
  }

  function confirmRename() {
    const layerId = currentLayer.value?.id
    if (layerId) {
      renameLayer(layerId, {
        name: renameDraftName.value,
        description: renameDraftDescription.value,
      })
    }
    renameOpen.value = false
  }

  function deleteSelectedLayer() {
    if (deleteLayer(selectedLayerId.value)) {
      selectedLayerId.value = BASE_LAYER_ID
    }
  }

  const newLayerOpen = ref(false)
  const newLayerName = ref('')
  const newLayerDescription = ref('')

  function openNewLayer() {
    newLayerName.value = ''
    newLayerDescription.value = ''
    newLayerOpen.value = true
  }

  function confirmNewLayer() {
    const id = createLayer({
      name: newLayerName.value,
      description: newLayerDescription.value,
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
    removeExtra,
    renameOpen,
    renameDraftName,
    renameDraftDescription,
    openRename,
    confirmRename,
    deleteSelectedLayer,
    newLayerOpen,
    newLayerName,
    newLayerDescription,
    openNewLayer,
    confirmNewLayer,
  }
}
