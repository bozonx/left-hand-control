export function useActionUsage(parseRef: (action: string | null | undefined) => string | null) {
  const { config } = useConfig()

  const usage = computed(() => {
    const byId: Record<string, string[]> = {}
    const note = (id: string, where: string) => {
      if (!byId[id]) byId[id] = []
      byId[id].push(where)
    }
    const noteIf = (action: string | null | undefined, where: string) => {
      const id = action ? parseRef(action) : null
      if (id) note(id, where)
    }
    for (const rule of config.value.rules) {
      const ruleLabel = rule.key || '?'
      noteIf(rule.tapAction, `rule ${ruleLabel} (tap)`)
      noteIf(rule.holdAction, `rule ${ruleLabel} (hold)`)
      noteIf(rule.doubleTapAction, `rule ${ruleLabel} (double-tap)`)
    }
    for (const [layerId, keymap] of Object.entries(config.value.layerKeymaps)) {
      for (const [code, action] of Object.entries(keymap.keys ?? {})) {
        noteIf(action, `${layerId}.${code}`)
      }
      for (const extra of keymap.extras ?? []) {
        noteIf(extra.action, `${layerId}.${extra.key || 'extra'}`)
      }
    }
    for (const macro of config.value.macros) {
      for (const [index, step] of macro.steps.entries()) {
        noteIf(step.keystroke, `macro ${macro.id} (#${index + 1})`)
      }
    }
    return byId
  })

  return { usage }
}
