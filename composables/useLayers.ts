import { randomId } from '~/utils/keys'

export function useLayers() {
  const { config } = useConfig()

  function ensureLayerKeymap(id: string) {
    if (!config.value.layerKeymaps[id]) {
      config.value.layerKeymaps[id] = { keys: {}, extras: [] }
    }
    return config.value.layerKeymaps[id]
  }

  function createLayer(input: {
    name: string
    description?: string
  }) {
    const name = input.name.trim()
    if (!name) return null
    const id = randomId()
    config.value.layers.push({
      id,
      name,
      description: input.description?.trim() || undefined,
    })
    ensureLayerKeymap(id)
    return id
  }

  function renameLayer(
    id: string,
    input: {
      name: string
      description?: string
    },
  ) {
    const layer = config.value.layers.find((item) => item.id === id)
    if (!layer) return false
    layer.name = input.name.trim() || layer.name
    layer.description = input.description?.trim() || undefined
    return true
  }

  function cloneLayer(id: string, newName: string) {
    const source = config.value.layers.find((layer) => layer.id === id)
    if (!source) return null
    const name = newName.trim()
    if (!name) return null
    const newId = randomId()
    config.value.layers.push({
      id: newId,
      name,
      description: source.description,
    })
    const sourceKeymap = config.value.layerKeymaps[id]
    if (sourceKeymap) {
      config.value.layerKeymaps[newId] = {
        keys: { ...sourceKeymap.keys },
        extras: sourceKeymap.extras.map((e) => ({ ...e, id: randomId() })),
      }
    } else {
      ensureLayerKeymap(newId)
    }
    return newId
  }

  function deleteLayer(id: string) {
    if (!config.value.layers.some((layer) => layer.id === id)) return false
    config.value.layers = config.value.layers.filter((layer) => layer.id !== id)
    delete config.value.layerKeymaps[id]
    config.value.rules = config.value.rules.map((rule) =>
      rule.layerId === id ? { ...rule, layerId: '' } : rule,
    )
    return true
  }

  return {
    ensureLayerKeymap,
    createLayer,
    renameLayer,
    cloneLayer,
    deleteLayer,
  }
}
