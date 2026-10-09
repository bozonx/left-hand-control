import { VISUAL_KEY_CODES } from '~/utils/keys'
import { STATIC_CATEGORIES } from '~/utils/actionCategories'
import { isSingleKeyAction } from '~/utils/actionSyntax'
import type { Layer, LayerKeymap } from '~/types/config'

const catalog = STATIC_CATEGORIES.flatMap((category) => category.items.map((item) => item.value))

export function useKeymapExtras(
  currentKeymap: ComputedRef<LayerKeymap>,
  currentLayer: ComputedRef<Layer | undefined>,
) {
  const extraKeys = computed(() => Object.entries(currentKeymap.value.keys)
    .filter(([key]) => !VISUAL_KEY_CODES.includes(key))
    .sort(([a], [b]) => {
      const rank = (key: string) => {
        const index = catalog.indexOf(key)
        return index < 0 ? catalog.length : index
      }
      return rank(a) - rank(b) || a.localeCompare(b)
    })
    .map(([key, action]) => ({ key, action })))

  function addExtra(key: string) {
    if (!currentLayer.value || !isSingleKeyAction(key) || VISUAL_KEY_CODES.includes(key)) return
    if (Object.hasOwn(currentKeymap.value.keys, key)) return
    currentKeymap.value.keys[key] = key
  }

  function removeExtra(key: string) {
    if (!currentLayer.value || VISUAL_KEY_CODES.includes(key)) return
    delete currentKeymap.value.keys[key]
  }

  function updateExtra(key: string, field: 'key' | 'action', value: string | null) {
    if (!currentLayer.value || VISUAL_KEY_CODES.includes(key)) return
    const keys = currentKeymap.value.keys
    if (!Object.hasOwn(keys, key)) return
    if (field === 'key') {
      if (!value || !isSingleKeyAction(value) || VISUAL_KEY_CODES.includes(value) || Object.hasOwn(keys, value)) return
      keys[value] = keys[key]!
      delete keys[key]
    } else if (value === '') {
      delete keys[key]
    } else {
      keys[key] = value
    }
  }

  return { extraKeys, addExtra, removeExtra, updateExtra }
}
