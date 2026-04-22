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

  function moveRule(id: string, direction: 'up' | 'down') {
    const index = config.value.rules.findIndex((r) => r.id === id)
    if (index === -1) return
    const newIndex = direction === 'up' ? index - 1 : index + 1
    if (newIndex < 0 || newIndex >= config.value.rules.length) return
    const [rule] = config.value.rules.splice(index, 1)
    config.value.rules.splice(newIndex, 0, rule)
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
    moveRule,
    newLayerOpen,
    newLayerName,
    newLayerDescription,
    openNewLayer,
    confirmNewLayer,
  }
}
