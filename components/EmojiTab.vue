<script setup lang="ts">
import type { EmojiHotkey, EmojiPage } from '~/types/config'
import {
    EMOJI_HOTKEYS,
    EMOJI_HOTKEY_LABELS,
    MAX_EMOJI_CELL_LENGTH,
    createDefaultEmojiPage,
} from '~/types/config'
import { EMOJI_CATALOG } from '~/utils/emojiCatalog'
import Sortable, { type SortableEvent } from 'sortablejs'

const { config } = useConfig()
const { t } = useI18n()
const toast = useToast()

const selectedPageIndex = ref(0)
const selectedKey = ref<EmojiHotkey>('KeyQ')
const customEmoji = ref('')
const confirmDeletePageOpen = ref(false)
const pendingDeletePageIndex = ref<number | null>(null)
const deletePageConfirm = ref<{ $el?: HTMLButtonElement } | null>(null)
const emojiGridRef = ref<HTMLElement | null>(null)
let emojiSortable: Sortable | null = null

const pages = computed(() => config.value.emojiPages || [])
const selectedPage = computed(
    () => pages.value[selectedPageIndex.value] ?? null,
)
const selectedValue = computed(
    () => selectedPage.value?.cells[selectedKey.value] ?? '',
)

function cellContentClass(value: string | undefined): string {
    if (!value) return 'text-3xl leading-none'
    const len = [...value].length
    if (len <= 2) return 'text-3xl leading-none'
    if (len <= 6) return 'text-base leading-snug'
    return 'text-xs leading-snug break-all'
}

function ensurePages() {
    if (!config.value.emojiPages || config.value.emojiPages.length === 0) {
        config.value.emojiPages = [createDefaultEmojiPage()]
    }
    if (selectedPageIndex.value >= config.value.emojiPages.length) {
        selectedPageIndex.value = Math.max(
            0,
            config.value.emojiPages.length - 1,
        )
    }
}

function createPage(): EmojiPage {
    const n = (config.value.emojiPages?.length ?? 0) + 1
    return {
        id: crypto.randomUUID(),
        name: t('emoji.pageName', { n }),
        cells: {},
    }
}

function addPage() {
    ensurePages()
    config.value.emojiPages.push(createPage())
    selectedPageIndex.value = config.value.emojiPages.length - 1
    selectedKey.value = 'KeyQ'
}

function removePage(index: number) {
    ensurePages()
    if (config.value.emojiPages.length <= 1) {
        config.value.emojiPages = [createPage()]
        selectedPageIndex.value = 0
        return
    }
    config.value.emojiPages.splice(index, 1)
    selectedPageIndex.value = Math.min(
        selectedPageIndex.value,
        config.value.emojiPages.length - 1,
    )
}

function askRemovePage(index: number) {
    pendingDeletePageIndex.value = index
    confirmDeletePageOpen.value = true
}

function confirmRemovePage() {
    if (pendingDeletePageIndex.value !== null) {
        removePage(pendingDeletePageIndex.value)
    }
    pendingDeletePageIndex.value = null
    confirmDeletePageOpen.value = false
}

function cancelRemovePage() {
    pendingDeletePageIndex.value = null
    confirmDeletePageOpen.value = false
}

function setCell(value: string) {
    ensurePages()
    const page = config.value.emojiPages[selectedPageIndex.value]
    if (!page) return
    const next = value.trim()
    if (next.length > MAX_EMOJI_CELL_LENGTH) {
        toast.add({
            title: t('emoji.cellTooLongTitle'),
            description: t('emoji.cellTooLongBody', {
                max: MAX_EMOJI_CELL_LENGTH,
            }),
            color: 'warning',
        })
        return
    }
    if (next) {
        page.cells[selectedKey.value] = next
    } else {
        delete page.cells[selectedKey.value]
    }
}

function clearCell() {
    setCell('')
}

function moveSelectedEmojiKey(fromIndex: number, toIndex: number) {
    const selectedIndex = EMOJI_HOTKEYS.indexOf(selectedKey.value)
    if (selectedIndex === fromIndex) {
        selectedKey.value = EMOJI_HOTKEYS[toIndex] ?? selectedKey.value
    } else if (fromIndex < selectedIndex && selectedIndex <= toIndex) {
        selectedKey.value = EMOJI_HOTKEYS[selectedIndex - 1] ?? selectedKey.value
    } else if (toIndex <= selectedIndex && selectedIndex < fromIndex) {
        selectedKey.value = EMOJI_HOTKEYS[selectedIndex + 1] ?? selectedKey.value
    }
}

function moveEmojiCellWithinPage(fromIndex: number, toIndex: number) {
    if (fromIndex === toIndex) return
    ensurePages()
    const page = config.value.emojiPages[selectedPageIndex.value]
    if (!page) return

    const values = EMOJI_HOTKEYS.map((key) => page.cells[key] ?? '')
    const [item] = values.splice(fromIndex, 1)
    values.splice(toIndex, 0, item ?? '')

    for (const [index, key] of EMOJI_HOTKEYS.entries()) {
        const value = values[index]?.trim() ?? ''
        if (value) {
            page.cells[key] = value
        } else {
            delete page.cells[key]
        }
    }
    moveSelectedEmojiKey(fromIndex, toIndex)
}

function initEmojiSortable() {
    if (emojiSortable || !emojiGridRef.value) return
    emojiSortable = Sortable.create(emojiGridRef.value, {
        animation: 150,
        handle: '.emoji-drag-handle',
        draggable: '[data-sortable-cell]',
        ghostClass: 'opacity-50',
        onEnd(event: SortableEvent) {
            if (event.oldIndex === undefined || event.newIndex === undefined) return
            moveEmojiCellWithinPage(event.oldIndex, event.newIndex)
        },
    })
}

watch(
    selectedValue,
    (value) => {
        customEmoji.value = value
    },
    { immediate: true },
)

watch(confirmDeletePageOpen, async (open) => {
    if (!open) return
    await nextTick()
    deletePageConfirm.value?.$el?.focus()
})

onMounted(ensurePages)
onMounted(async () => {
    await nextTick()
    initEmojiSortable()
})
onBeforeUnmount(() => {
    emojiSortable?.destroy()
    emojiSortable = null
})
</script>

<template>
    <div class="space-y-4">
        <UCard>
            <template #header>
                <div class="flex items-center justify-between gap-3">
                    <div>
                        <h2 class="text-sm font-semibold">
                            {{ $t('emoji.title') }}
                        </h2>
                        <p class="mt-0.5 text-xs text-(--ui-text-muted)">
                            {{ $t('emoji.subtitle') }}
                        </p>
                    </div>
                    <UButton
                        icon="i-lucide-plus"
                        size="sm"
                        class="whitespace-nowrap"
                        @click="addPage"
                    >
                        {{ $t('emoji.addPage') }}
                    </UButton>
                </div>
            </template>

            <div
                class="grid items-start gap-4 lg:grid-cols-[minmax(0,1fr)_320px]"
            >
                <div class="space-y-4">
                    <div class="flex flex-wrap gap-2">
                        <div
                            class="flex h-8 items-center px-1 text-xs font-medium text-(--ui-text-muted)"
                        >
                            {{ $t('common.page') }}
                        </div>
                        <UButton
                            v-for="(page, index) in pages"
                            :key="page.id"
                            :color="
                                index === selectedPageIndex
                                    ? 'primary'
                                    : 'neutral'
                            "
                            :variant="
                                index === selectedPageIndex ? 'soft' : 'outline'
                            "
                            size="sm"
                            @click="selectedPageIndex = index"
                        >
                            {{ page.name }}
                        </UButton>
                    </div>

                    <div
                        v-if="selectedPage"
                        ref="emojiGridRef"
                        class="grid grid-cols-5 gap-2 rounded-lg border border-(--ui-border-muted) bg-(--ui-bg-elevated) p-3"
                    >
                        <button
                            v-for="key in EMOJI_HOTKEYS"
                            :key="key"
                            type="button"
                            data-sortable-cell
                            class="flex aspect-square min-h-20 flex-col items-center justify-center gap-2 rounded-md border bg-(--ui-bg) p-2 transition hover:border-primary hover:bg-primary/10"
                            :class="
                                selectedKey === key
                                    ? 'border-primary ring-1 ring-primary/35'
                                    : 'border-(--ui-border-muted)'
                            "
                            @click="selectedKey = key"
                        >
                            <span class="flex w-full items-center justify-between gap-2">
                                <span
                                    class="font-mono text-xs uppercase text-(--ui-primary)"
                                    >{{ EMOJI_HOTKEY_LABELS[key] }}</span
                                >
                                <span
                                    class="emoji-drag-handle cursor-grab rounded p-0.5 text-(--ui-text-muted) hover:text-(--ui-primary) active:cursor-grabbing"
                                    :aria-label="$t('common.drag')"
                                    @click.stop
                                >
                                    <UIcon name="i-lucide-grip" class="h-3.5 w-3.5" />
                                </span>
                            </span>
                            <span
                                class="overflow-hidden text-center"
                                :class="
                                    cellContentClass(selectedPage.cells[key])
                                "
                                >{{ selectedPage.cells[key] || '＋' }}</span
                            >
                        </button>
                    </div>
                </div>

                <div v-if="selectedPage" class="space-y-4">
                    <UCard>
                        <template #header>
                            <div
                                class="flex items-center justify-between gap-2"
                            >
                                <UFormField class="min-w-0 flex-1">
                                    <template #label>
                                        <FieldLabel
                                            :label="$t('common.page')"
                                        />
                                    </template>
                                    <UInput
                                        v-model="selectedPage.name"
                                        size="sm"
                                        :aria-label="$t('emoji.pageLabel')"
                                        class="w-full"
                                    />
                                </UFormField>
                                <UButton
                                    icon="i-lucide-trash-2"
                                    color="neutral"
                                    variant="ghost"
                                    size="sm"
                                    class="self-end"
                                    :title="$t('emoji.deletePageTitle')"
                                    :aria-label="$t('emoji.deletePage')"
                                    @click="askRemovePage(selectedPageIndex)"
                                />
                            </div>
                        </template>

                        <div class="space-y-3">
                            <div>
                                <FieldLabel
                                    :label="
                                        $t('emoji.cellLabel', {
                                            key: EMOJI_HOTKEY_LABELS[
                                                selectedKey
                                            ],
                                        })
                                    "
                                />
                                <div class="mt-1 flex gap-2">
                                    <UInput
                                        v-model="customEmoji"
                                        class="min-w-0 flex-1"
                                        :placeholder="
                                            $t('emoji.customPlaceholder')
                                        "
                                        @keydown.enter.prevent="
                                            setCell(customEmoji)
                                        "
                                        @blur="setCell(customEmoji)"
                                    />
                                    <UButton
                                        icon="i-lucide-eraser"
                                        color="neutral"
                                        variant="ghost"
                                        :aria-label="$t('emoji.clearCell')"
                                        @click="clearCell"
                                    />
                                </div>
                            </div>

                            <div
                                class="max-h-[26rem] space-y-4 overflow-y-auto pr-1"
                            >
                                <section
                                    v-for="category in EMOJI_CATALOG"
                                    :key="category.id"
                                    class="space-y-2"
                                >
                                    <h3
                                        class="sticky top-0 z-10 bg-(--ui-bg) py-1 text-xs font-semibold uppercase tracking-wide text-(--ui-text-muted)"
                                    >
                                        {{ $t(category.labelKey) }}
                                    </h3>
                                    <div class="grid grid-cols-6 gap-1.5">
                                        <button
                                            v-for="item in category.items"
                                            :key="`${category.id}-${item}`"
                                            type="button"
                                            class="flex aspect-square items-center justify-center rounded-md border border-(--ui-border-muted) bg-(--ui-bg) text-xl transition hover:border-primary hover:bg-primary/10"
                                            @click="setCell(item)"
                                        >
                                            {{ item }}
                                        </button>
                                    </div>
                                </section>
                            </div>
                        </div>
                    </UCard>
                </div>
            </div>
        </UCard>

        <UModal
            v-model:open="confirmDeletePageOpen"
            :title="$t('emoji.confirmDeletePageTitle')"
        >
            <template #body>
                <p class="text-sm">{{ $t('emoji.confirmDeletePageBody') }}</p>
            </template>
            <template #footer>
                <div class="flex w-full justify-end gap-2">
                    <UButton
                        color="neutral"
                        variant="ghost"
                        @click="cancelRemovePage"
                    >
                        {{ $t('common.cancel') }}
                    </UButton>
                    <UButton
                        ref="deletePageConfirm"
                        color="error"
                        icon="i-lucide-trash-2"
                        @click="confirmRemovePage"
                    >
                        {{ $t('common.delete') }}
                    </UButton>
                </div>
            </template>
        </UModal>
    </div>
</template>
