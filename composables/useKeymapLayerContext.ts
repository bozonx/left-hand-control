import type { LayerKeymap } from '~/types/config'
import type { KeyLabelMode } from '~/utils/keys'

const EMPTY_KEYMAP: LayerKeymap = {
  keys: {},
  extras: [],
}

export function useKeymapLayerContext() {
  const { config } = useConfig()
  const uiState = useUiState()
  const { ensureLayerKeymap } = useLayers()

  const selectedLayerId = computed<string>({
    get: () => {
      const selected = uiState.state.value.selectedLayerId
      if (selected && config.value.layers.some((layer) => layer.id === selected)) {
        return selected
      }
      return config.value.layers[0]?.id ?? ''
    },
    set: (value) => {
      uiState.setSelectedLayerId(value)
    },
  })

  const keyLabelMode = computed<KeyLabelMode>({
    get: () => uiState.state.value.keyLabelMode,
    set: (value) => {
      uiState.setKeyLabelMode(value)
    },
  })

  watch(
    () => config.value.layers.map((layer) => layer.id).join(','),
    () => {
      if (!config.value.layers.some((layer) => layer.id === selectedLayerId.value)) {
        uiState.setSelectedLayerId(config.value.layers[0]?.id ?? '')
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

  const currentKeymap = computed(() => {
    if (!currentLayer.value) return EMPTY_KEYMAP
    return ensureLayerKeymap(selectedLayerId.value)
  })

  return {
    config,
    selectedLayerId,
    keyLabelMode,
    layerItems,
    currentLayer,
    currentKeymap,
  }
}
