import { nextTick, ref, type Ref } from 'vue'

export function useMenuPage(pages: Ref<unknown[]>) {
  const pageIndex = ref(0)
  const scrollEl = ref<HTMLElement | null>(null)
  const pageEls = ref<HTMLElement[]>([])
  let scrollFrame = 0

  function wait(ms: number) {
    return new Promise<void>((resolve) => window.setTimeout(resolve, ms))
  }

  function nextPage() {
    setPage((pageIndex.value + 1) % Math.max(1, pages.value.length))
  }

  function prevPage() {
    setPage(
      (pageIndex.value - 1 + Math.max(1, pages.value.length)) %
        Math.max(1, pages.value.length),
    )
  }

  function setPage(index: number) {
    const next = Math.min(
      Math.max(index, 0),
      Math.max(0, pages.value.length - 1),
    )
    pageIndex.value = next
    pageEls.value[next]?.scrollIntoView({ block: 'start', behavior: 'smooth' })
  }

  function setPageRef(el: unknown, index: number) {
    if (el instanceof HTMLElement) {
      pageEls.value[index] = el
    }
  }

  function updatePageFromScroll() {
    const scroller = scrollEl.value
    if (!scroller) return
    const next = Math.round(
      scroller.scrollTop / Math.max(1, scroller.clientHeight),
    )
    pageIndex.value = Math.min(
      Math.max(next, 0),
      Math.max(0, pages.value.length - 1),
    )
  }

  function onScroll() {
    if (scrollFrame) window.cancelAnimationFrame(scrollFrame)
    scrollFrame = window.requestAnimationFrame(updatePageFromScroll)
  }

  async function resetScroll(index = 0) {
    const next = Math.min(
      Math.max(index, 0),
      Math.max(0, pages.value.length - 1),
    )
    pageIndex.value = next
    pageEls.value = []
    await nextTick()
    scrollEl.value?.scrollTo({
      top: scrollEl.value.clientHeight * next,
    })
  }

  function handleTabKey(e: KeyboardEvent): boolean {
    if (e.key !== 'Tab') return false
    e.preventDefault()
    if (e.shiftKey) prevPage()
    else nextPage()
    return true
  }

  function cleanup() {
    if (scrollFrame) window.cancelAnimationFrame(scrollFrame)
    pageEls.value = []
  }

  return {
    pageIndex,
    scrollEl,
    pageEls,
    wait,
    nextPage,
    prevPage,
    setPage,
    setPageRef,
    onScroll,
    resetScroll,
    handleTabKey,
    cleanup,
  }
}
