<script setup lang="ts">
import { emit, listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import {
    EMOJI_HOTKEYS,
    EMOJI_HOTKEY_LABELS,
    createDefaultEmojiPage,
} from '~/types/config'
import type { EmojiHotkey } from '~/types/config'

const { config, load } = useConfig()
const toast = useToast()
const { t } = useI18n()

const pageIndex = ref(0)
const scrollEl = ref<HTMLElement | null>(null)
const pageEls = ref<HTMLElement[]>([])
const pages = computed(() => {
    const configured = config.value.emojiPages || []
    return configured.length > 0 ? configured : [createDefaultEmojiPage()]
})
const page = computed(
    () =>
        pages.value[pageIndex.value] ??
        pages.value[0] ??
        createDefaultEmojiPage(),
)

let unlistenShow: (() => void) | null = null
let scrollFrame = 0

function wait(ms: number) {
    return new Promise((resolve) => window.setTimeout(resolve, ms))
}

async function closeMenu() {
    await emit('hide_emoji_menu').catch((e) => {
        logger.error('Failed to emit hide emoji menu', e)
    })
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
    pageIndex.value = index
    pageEls.value[index]?.scrollIntoView({ block: 'start', behavior: 'smooth' })
}

function setPageRef(el: unknown, index: number) {
    if (el instanceof HTMLElement) {
        pageEls.value[index] = el
    }
}

function updatePageFromScroll() {
    const scroller = scrollEl.value
    if (!scroller) return
    const next = Math.round(scroller.scrollTop / Math.max(1, scroller.clientHeight))
    pageIndex.value = Math.min(Math.max(next, 0), Math.max(0, pages.value.length - 1))
}

function onScroll() {
    if (scrollFrame) window.cancelAnimationFrame(scrollFrame)
    scrollFrame = window.requestAnimationFrame(updatePageFromScroll)
}

async function applyEmoji(emoji: string | undefined) {
    if (!emoji) return
    await closeMenu()
    await wait(80)
    try {
        await invoke('insert_text', { text: emoji })
    } catch (e) {
        logger.error('Failed to insert emoji', e)
        toast.add({
            title: t('emojiMenu.insertFailedTitle'),
            description: t('emojiMenu.insertFailedBody'),
            color: 'error',
        })
    }
}

function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
        e.preventDefault()
        void closeMenu()
        return
    }
    if (e.key === 'Tab') {
        e.preventDefault()
        if (e.shiftKey) {
            prevPage()
        } else {
            nextPage()
        }
        return
    }
    const code = e.code
    if ((EMOJI_HOTKEYS as readonly string[]).includes(code)) {
        e.preventDefault()
        void applyEmoji(page.value.cells[code as EmojiHotkey])
    }
}

onMounted(async () => {
    await load()

    unlistenShow = await listen('show_emoji_menu', async () => {
        await load()
        pageIndex.value = 0
        await nextTick()
        scrollEl.value?.scrollTo({ top: 0 })
    })

    window.addEventListener('keydown', onKeydown)
})

onBeforeUnmount(() => {
    window.removeEventListener('keydown', onKeydown)
    if (scrollFrame) window.cancelAnimationFrame(scrollFrame)
    unlistenShow?.()
})
</script>

<template>
    <div
        class="flex h-screen w-screen select-none items-center justify-center overflow-hidden bg-transparent p-4"
    >
        <div
            class="flex w-full max-w-[520px] flex-col rounded-lg border border-(--ui-border) bg-(--ui-bg-elevated)/95 p-3 shadow-2xl backdrop-blur-md"
        >
            <div class="mb-3 flex items-center justify-between gap-3">
                <div class="min-w-0">
                    <p class="truncate text-sm font-semibold">
                        {{ page.name }}
                    </p>
                    <p class="text-xs text-(--ui-text-muted)">
                        {{ $t('emojiMenu.tabHint') }}
                    </p>
                </div>
                <UBadge color="neutral" variant="outline" size="sm">
                    {{ pageIndex + 1 }} / {{ pages.length }}
                </UBadge>
            </div>

            <div
                ref="scrollEl"
                class="h-[296px] overflow-y-auto overscroll-contain scroll-smooth snap-y snap-mandatory rounded-md"
                @scroll="onScroll"
            >
                <section
                    v-for="(emojiPage, pageNumber) in pages"
                    :key="emojiPage.id"
                    :ref="(el) => setPageRef(el, pageNumber)"
                    class="flex h-full snap-start flex-col"
                >
                    <div class="grid grid-cols-5 gap-2">
                        <button
                            v-for="key in EMOJI_HOTKEYS"
                            :key="key"
                            type="button"
                            class="flex aspect-square min-h-20 flex-col items-center justify-center gap-2 rounded-md border border-(--ui-border-muted) bg-(--ui-bg) p-2 transition hover:border-primary hover:bg-primary/10 disabled:cursor-default disabled:opacity-40"
                            :disabled="!emojiPage.cells[key]"
                            @click="applyEmoji(emojiPage.cells[key])"
                        >
                            <span class="text-3xl leading-none">{{
                                emojiPage.cells[key] || ' '
                            }}</span>
                            <span
                                class="font-mono text-xs uppercase text-(--ui-text-muted)"
                                >{{ EMOJI_HOTKEY_LABELS[key] }}</span
                            >
                        </button>
                    </div>
                </section>
            </div>

            <div class="mt-2 flex items-center justify-center gap-1.5">
                <button
                    v-for="(_, index) in pages"
                    :key="index"
                    type="button"
                    class="h-1.5 rounded-full transition-all"
                    :class="
                        index === pageIndex
                            ? 'w-4 bg-primary'
                            : 'w-1.5 bg-(--ui-border-accented) hover:bg-(--ui-text-muted)'
                    "
                    @click="setPage(index)"
                />
            </div>

            <div class="mt-3 border-t border-(--ui-border-muted) pt-2">
                <button
                    type="button"
                    class="flex h-10 w-full items-center justify-center rounded-md px-3 text-sm font-medium text-(--ui-text-muted) transition-colors hover:bg-(--ui-bg-accented) hover:text-(--ui-text)"
                    @click="closeMenu"
                >
                    {{ $t('common.cancel') }}
                </button>
            </div>
        </div>
    </div>
</template>
