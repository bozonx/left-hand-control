type AppShellScrollContext = {
  isScrolled: Readonly<Ref<boolean>>
  scrollToTop: () => void
}

const APP_SHELL_SCROLL_KEY = 'app-shell-scroll'

export function useAppShellScroll(): AppShellScrollContext {
  return inject<AppShellScrollContext>(APP_SHELL_SCROLL_KEY, {
    isScrolled: readonly(ref(false)),
    scrollToTop: () => {},
  })
}
