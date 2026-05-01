import { randomId } from '~/utils/keys'

export function useEntityEditor<T extends { id: string }>(items: Ref<T[]>) {
  const uiKeys = new WeakMap<T, string>()

  function newId(base?: string): string {
    const arr = items.value
    if (base && !arr.some((item) => item.id === base)) return base
    if (base) {
      for (let i = 2; i < 1000; i += 1) {
        const candidate = `${base}${i}`
        if (!arr.some((item) => item.id === candidate)) return candidate
      }
    }
    let id = ''
    do {
      id = randomId()
    } while (arr.some((item) => item.id === id))
    return id
  }

  function uiKeyOf(entity: T): string {
    let key = uiKeys.get(entity)
    if (!key) {
      key = randomId()
      uiKeys.set(entity, key)
    }
    return key
  }

  function moveEntity(uiKey: string, delta: number) {
    const index = items.value.findIndex((item) => uiKeyOf(item) === uiKey)
    const next = index + delta
    if (index < 0 || next < 0 || next >= items.value.length) return
    const arr = items.value.slice()
    const [item] = arr.splice(index, 1) as [T]
    arr.splice(next, 0, item)
    items.value = arr
  }

  return { newId, uiKeyOf, moveEntity }
}
