import { randomId } from '~/utils/keys'
import type { Layer, LayerKeymap } from '~/types/config'

export function useKeymapExtras(
  currentKeymap: ComputedRef<LayerKeymap>,
  currentLayer: ComputedRef<Layer | undefined>,
) {
  function addExtra() {
    if (!currentLayer.value) return
    currentKeymap.value.extras.unshift({
      id: randomId(),
      key: '',
      action: '',
    })
  }

  function moveExtra(id: string, direction: 'up' | 'down') {
    if (!currentLayer.value) return
    const index = currentKeymap.value.extras.findIndex((extra) => extra.id === id)
    if (index === -1) return
    const newIndex = direction === 'up' ? index - 1 : index + 1
    if (newIndex < 0 || newIndex >= currentKeymap.value.extras.length) return
    const [extra] = currentKeymap.value.extras.splice(index, 1)
    if (!extra) return
    currentKeymap.value.extras.splice(newIndex, 0, extra)
  }

  function removeExtra(id: string) {
    if (!currentLayer.value) return
    currentKeymap.value.extras = currentKeymap.value.extras.filter(
      (extra) => extra.id !== id,
    )
  }

  function updateExtra(id: string, field: 'key' | 'action', value: string) {
    if (!currentLayer.value) return
    const extra = currentKeymap.value.extras.find((e) => e.id === id)
    if (!extra) return
    if (field === 'key') extra.key = value
    else extra.action = value
  }

  return {
    addExtra,
    moveExtra,
    removeExtra,
    updateExtra,
  }
}
