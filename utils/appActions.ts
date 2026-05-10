export interface AppAction {
  id: string
  nameKey: string
  nameParams?: Record<string, string | number>
  hint?: string
}

export const APP_ACTIONS: AppAction[] = Array.from({ length: 5 }, (_, i) => {
  const n = i + 1
  return [
    {
      id: `showQuickMenu${n}`,
      nameKey: 'appActions.showQuickMenu',
      nameParams: { n },
      hint: `Show Quick Actions menu page ${n} at screen center`,
    },
    {
      id: `showEmojiMenu${n}`,
      nameKey: 'appActions.showEmojiMenu',
      nameParams: { n },
      hint: `Show Emoji & Symbols menu page ${n} at screen center`,
    },
  ]
}).flat()

const BY_ID: Record<string, AppAction> = Object.fromEntries(
  APP_ACTIONS.map((a) => [a.id, a]),
)

export function appActionById(id: string): AppAction | undefined {
  return BY_ID[id]
}
