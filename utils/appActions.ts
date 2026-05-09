export interface AppAction {
  id: string
  nameKey: string
  hint?: string
}

export const APP_ACTIONS: AppAction[] = [
  {
    id: 'showQuickMenu',
    nameKey: 'appActions.showQuickMenu',
    hint: 'Show Quick Actions menu at screen center',
  },
  {
    id: 'showEmojiMenu',
    nameKey: 'appActions.showEmojiMenu',
    hint: 'Show Emoji & Symbols menu at screen center',
  },
]

const BY_ID: Record<string, AppAction> = Object.fromEntries(
  APP_ACTIONS.map((a) => [a.id, a]),
)

export function appActionById(id: string): AppAction | undefined {
  return BY_ID[id]
}
