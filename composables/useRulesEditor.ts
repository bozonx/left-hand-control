import { randomId } from '~/utils/keys'

export function useRulesEditor() {
  const { config } = useConfig()
  const { createLayer } = useLayers()

  const layerOptions = computed(() =>
    config.value.layers
      .filter((layer) => layer.id !== 'base')
      .map((layer) => ({ label: layer.name, value: layer.id })),
  )

  const newLayerOpen = ref(false)
  const newLayerName = ref('')
  const newLayerDescription = ref('')
  const newLayerForRuleId = ref<string | null>(null)

  function addRule() {
    config.value.rules.push({
      id: randomId(),
      key: '',
      layerId: '',
      tapAction: '',
      holdAction: '',
      doubleTapAction: '',
      holdTimeoutMs: undefined,
      doubleTapTimeoutMs: undefined,
    })
  }

  function removeRule(id: string) {
    config.value.rules = config.value.rules.filter((rule) => rule.id !== id)
  }

  function openNewLayer(ruleId: string) {
    newLayerForRuleId.value = ruleId
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
    if (newLayerForRuleId.value) {
      const rule = config.value.rules.find(
        (item) => item.id === newLayerForRuleId.value,
      )
      if (rule) rule.layerId = id
    }
    newLayerOpen.value = false
  }

  return {
    config,
    layerOptions,
    addRule,
    removeRule,
    newLayerOpen,
    newLayerName,
    newLayerDescription,
    openNewLayer,
    confirmNewLayer,
  }
}
