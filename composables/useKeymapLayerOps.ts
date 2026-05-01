import type { AppConfig, Layer, LayerKeymap } from '~/types/config'

export function useKeymapLayerOps(
  selectedLayerId: Ref<string>,
  currentLayer: ComputedRef<Layer | undefined>,
  config: Ref<AppConfig>,
) {
  const toast = useToast()
  const { t } = useI18n()
  const { renameLayer, createLayer, cloneLayer, deleteLayer } = useLayers()

  const renameOpen = ref(false)
  const renameDraftName = ref('')
  const renameDraftDescription = ref('')
  const deleteConfirmOpen = ref(false)

  const affectedRulesCount = computed(() =>
    config.value.rules.filter((rule: { layerId: string }) => rule.layerId === selectedLayerId.value).length,
  )

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
      selectedLayerId.value = config.value.layers[0]?.id ?? ''
      deleteConfirmOpen.value = false
    }
  }

  function requestDeleteSelectedLayer() {
    if (!currentLayer.value) return
    deleteConfirmOpen.value = true
  }

  function cancelDeleteSelectedLayer() {
    deleteConfirmOpen.value = false
  }

  const clearConfirmOpen = ref(false)
  const lastClearedBackup = ref<LayerKeymap | null>(null)

  function requestClearSelectedLayer() {
    if (!currentLayer.value) return
    clearConfirmOpen.value = true
  }

  function cancelClearSelectedLayer() {
    clearConfirmOpen.value = false
  }

  function clearSelectedLayer() {
    if (!currentLayer.value) return
    const keymap = config.value.layerKeymaps[selectedLayerId.value]
    if (!keymap) return
    lastClearedBackup.value = {
      keys: { ...keymap.keys },
      extras: keymap.extras.map((e: { id: string; key: string; action: string }) => ({ ...e })),
    }
    keymap.keys = {}
    keymap.extras = []
    clearConfirmOpen.value = false
    toast.add({
      title: t('keymap.layerCleared'),
      color: 'success',
      icon: 'i-lucide-check',
      actions: [
        {
          label: t('keymap.undoClear'),
          onClick: undoClear,
        },
      ],
    })
  }

  function undoClear() {
    const backup = lastClearedBackup.value
    if (!backup || !currentLayer.value) return
    const keymap = config.value.layerKeymaps[selectedLayerId.value]
    if (!keymap) return
    keymap.keys = backup.keys
    keymap.extras = backup.extras
    lastClearedBackup.value = null
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

  const cloneLayerOpen = ref(false)
  const cloneDraftName = ref('')

  function openCloneLayer() {
    const layer = currentLayer.value
    cloneDraftName.value = layer ? `${layer.name} copy` : ''
    cloneLayerOpen.value = true
  }

  function confirmCloneLayer() {
    const id = cloneLayer(selectedLayerId.value, cloneDraftName.value)
    if (!id) return
    selectedLayerId.value = id
    cloneLayerOpen.value = false
  }

  return {
    renameOpen,
    renameDraftName,
    renameDraftDescription,
    openRename,
    confirmRename,
    updateCurrentLayerDescription,
    affectedRulesCount,
    deleteConfirmOpen,
    requestDeleteSelectedLayer,
    cancelDeleteSelectedLayer,
    deleteSelectedLayer,
    clearConfirmOpen,
    lastClearedBackup,
    requestClearSelectedLayer,
    cancelClearSelectedLayer,
    clearSelectedLayer,
    newLayerOpen,
    newLayerName,
    newLayerDescription,
    openNewLayer,
    confirmNewLayer,
    cloneLayerOpen,
    cloneDraftName,
    openCloneLayer,
    confirmCloneLayer,
  }
}
