<script setup lang="ts">
import type { EmojiHotkey, EmojiPage } from '~/types/config'
import {
    EMOJI_HOTKEYS,
    EMOJI_HOTKEY_LABELS,
    MAX_EMOJI_CELL_LENGTH,
    STANDARD_EMOJIS,
    createDefaultEmojiPage,
} from '~/types/config'

const { config } = useConfig()
const { t } = useI18n()
const toast = useToast()

const selectedPageIndex = ref(0)
const selectedKey = ref<EmojiHotkey>('KeyQ')
const customEmoji = ref('')
const confirmDeletePageOpen = ref(false)
const pendingDeletePageIndex = ref<number | null>(null)
const deletePageConfirm = ref<{ $el?: HTMLButtonElement } | null>(null)

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
    customEmoji.value = ''
}

function clearCell() {
    setCell('')
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

            <div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
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
                        class="grid grid-cols-5 gap-2 rounded-lg border border-(--ui-border-muted) bg-(--ui-bg-elevated) p-3"
                    >
                        <button
                            v-for="key in EMOJI_HOTKEYS"
                            :key="key"
                            type="button"
                            class="flex aspect-square min-h-20 flex-col items-center justify-center gap-2 rounded-md border bg-(--ui-bg) p-2 transition hover:border-primary hover:bg-primary/10"
                            :class="
                                selectedKey === key
                                    ? 'border-primary ring-1 ring-primary/35'
                                    : 'border-(--ui-border-muted)'
                            "
                            @click="selectedKey = key"
                        >
                            <span
                                class="font-mono text-xs uppercase text-(--ui-primary)"
                            >{{
                                EMOJI_HOTKEY_LABELS[key]
                            }}</span>
                            <span
                                class="overflow-hidden text-center"
                                :class="cellContentClass(selectedPage.cells[key])"
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
                                        <FieldLabel :label="$t('common.page')" />
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

                            <div class="grid grid-cols-6 gap-1.5">
                                <button
                                    v-for="emoji in STANDARD_EMOJIS"
                                    :key="emoji"
                                    type="button"
                                    class="flex aspect-square items-center justify-center rounded-md border border-(--ui-border-muted) bg-(--ui-bg) text-xl transition hover:border-primary hover:bg-primary/10"
                                    @click="setCell(emoji)"
                                >
                                    {{ emoji }}
                                </button>
                            </div>
                        </div>
                    </UCard>
                </div>
            </div>
        </UCard>

        <UModal v-model:open="confirmDeletePageOpen" :title="$t('emoji.confirmDeletePageTitle')">
            <template #body>
                <p class="text-sm">{{ $t('emoji.confirmDeletePageBody') }}</p>
            </template>
            <template #footer>
                <div class="flex w-full justify-end gap-2">
                    <UButton color="neutral" variant="ghost" @click="cancelRemovePage">
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
