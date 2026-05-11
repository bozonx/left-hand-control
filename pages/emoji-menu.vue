<script setup lang="ts">
import { listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import {
    EMOJI_HOTKEYS,
    EMOJI_HOTKEY_LABELS,
    createDefaultEmojiPage,
} from '~/types/config'
import type { EmojiHotkey } from '~/types/config'
import { useMenuPage } from '~/composables/useMenuPage'

const { config, load } = useConfig()
const route = useRoute()
const toast = useToast()
const { t } = useI18n()

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

const {
    pageIndex,
    scrollEl,
    wait,
    setPage,
    setPageRef,
    onScroll,
    resetScroll,
    handleTabKey,
    cleanup,
} = useMenuPage(pages)

function cellContentClass(value: string | undefined): string {
    if (!value) return 'text-3xl leading-none'
    const len = [...value].length
    if (len <= 2) return 'text-3xl leading-none'
    if (len <= 6) return 'text-base leading-snug'
    return 'text-xs leading-snug break-all'
}

let unlistenShow: (() => void) | null = null
let menuGeneration = 0
let pendingHotkeyCode: string | null = null
let isKeydownListenerAttached = false
const isReady = ref(false)

function menuPageFromPayload(payload: unknown): number {
    const page =
        typeof payload === 'number'
            ? payload
            : Number.parseInt(String(payload), 10)
    return Number.isFinite(page) ? page - 1 : 0
}

async function closeMenu() {
    await invoke('hide_emoji_menu').catch((e) => {
        logger.error('Failed to hide emoji menu', e)
    })
}

async function prepareMenu(payload: unknown, clearPending = true) {
    const generation = ++menuGeneration
    const nextPage = menuPageFromPayload(payload)
    isReady.value = false
    pageIndex.value = nextPage
    if (clearPending) pendingHotkeyCode = null

    await load()
    await resetScroll(nextPage)

    if (generation !== menuGeneration) return
    isReady.value = true
    flushPendingHotkey()
}

async function applyEmoji(emoji: string | undefined) {
    if (!emoji) return
    await closeMenu()
    await wait(0)
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

function applyHotkey(code: string) {
    if (!isReady.value) {
        pendingHotkeyCode = code
        return
    }
    void applyEmoji(page.value.cells[code as EmojiHotkey])
}

function flushPendingHotkey() {
    const code = pendingHotkeyCode
    pendingHotkeyCode = null
    if (code) applyHotkey(code)
}

function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
        e.preventDefault()
        void closeMenu()
        return
    }
    if (handleTabKey(e)) return
    const code = e.code
    if ((EMOJI_HOTKEYS as readonly string[]).includes(code)) {
        e.preventDefault()
        applyHotkey(code)
    }
}

function attachKeydownListener() {
    if (!import.meta.client || isKeydownListenerAttached) return
    window.addEventListener('keydown', onKeydown, true)
    isKeydownListenerAttached = true
}

function detachKeydownListener() {
    if (!import.meta.client || !isKeydownListenerAttached) return
    window.removeEventListener('keydown', onKeydown, true)
    isKeydownListenerAttached = false
}

attachKeydownListener()

onMounted(async () => {
    attachKeydownListener()
    unlistenShow = await listen('open_emoji_menu_page', async (event) => {
        await prepareMenu(event.payload)
    })

    await prepareMenu(route.query.page, false)
})

onBeforeUnmount(() => {
    detachKeydownListener()
    cleanup()
    unlistenShow?.()
})
</script>

<template>
    <div
        class="flex h-screen w-screen select-none items-center justify-center overflow-hidden bg-transparent p-4"
    >
        <div
            class="flex w-full max-w-[520px] flex-col rounded-lg border border-(--ui-border) bg-(--ui-bg-elevated) p-3 shadow-2xl"
        >
            <div class="mb-3 flex items-center justify-between gap-3">
                <div class="min-w-0">
                    <p class="truncate text-sm font-semibold">
                        {{ page.name }}
                    </p>
                    <p
                        v-if="pages.length > 1"
                        class="text-xs text-(--ui-text-muted)"
                    >
                        {{ $t('emojiMenu.tabHint') }}
                    </p>
                    <p class="text-xs text-(--ui-text-muted)">
                        {{ $t('emojiMenu.keysHint') }}
                    </p>
                </div>
                <UBadge
                    v-if="pages.length > 1"
                    color="neutral"
                    variant="outline"
                    size="sm"
                    class="shrink-0 whitespace-nowrap"
                >
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
                            <span
                                class="font-mono text-xs uppercase text-(--ui-text-muted)"
                                >{{ EMOJI_HOTKEY_LABELS[key] }}</span
                            >
                            <span
                                class="overflow-hidden text-center"
                                :class="cellContentClass(emojiPage.cells[key])"
                                >{{ emojiPage.cells[key] || ' ' }}</span
                            >
                        </button>
                    </div>
                </section>
            </div>

            <div
                v-if="pages.length > 1"
                class="mt-2 flex items-center justify-center gap-1.5"
            >
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
