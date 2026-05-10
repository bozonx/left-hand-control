<script setup lang="ts">
import InputWithClearButton from '~/components/shared/InputWithClearButton.vue'
import { parseTextAction, textActionRef } from '~/types/config'
import type { ActionItem } from '~/utils/actionCategories'

interface TextPart {
    text: string
    match: boolean
}

const props = defineProps<{
    activeCategory: string
    filteredItems: ActionItem[]
    keyOnly?: boolean
    selectionVersion?: number
    singleKeyOnly?: boolean
}>()

const emit = defineEmits<{
    pick: [value: string]
}>()

const draft = defineModel<string>({ default: '' })

const showSuggestions = ref(false)
const inputRef = ref<InstanceType<typeof InputWithClearButton> | null>(null)
const suggestionsRef = ref<HTMLElement | null>(null)
const captureButtonRef = ref<{ $el?: HTMLElement } | HTMLElement | null>(null)
const activeIndex = ref(-1)
const pickerId = useId()
const listboxId = `${pickerId}-listbox`
const dialogTitleId = `${pickerId}-capture-title`

const captureActive = ref(false)
const pressedCaptureKeys = ref<Set<string>>(new Set())
const capturedChordKeys = ref<Set<string>>(new Set())
const capturedDraft = ref('')
const captureOriginalDraft = ref('')

const isTextCategory = computed(() => props.activeCategory === 'text')
const textDraft = computed({
    get: () => parseTextAction(draft.value) ?? '',
    set: (value: string) => {
        draft.value = textActionRef(value)
    },
})

let focusoutTimer: ReturnType<typeof setTimeout> | null = null

const activeDescendantId = computed(() =>
    activeIndex.value >= 0
        ? `${pickerId}-option-${activeIndex.value}`
        : undefined,
)

const MODIFIER_CODES = new Set([
    'ControlLeft',
    'ControlRight',
    'ShiftLeft',
    'ShiftRight',
    'AltLeft',
    'AltRight',
    'MetaLeft',
    'MetaRight',
])

function highlightParts(text: string, query: string): TextPart[] {
    if (!query) return [{ text, match: false }]
    const lowerText = text.toLowerCase()
    const lowerQuery = query.toLowerCase()
    const parts: TextPart[] = []
    let i = 0
    while (i < text.length) {
        const idx = lowerText.indexOf(lowerQuery, i)
        if (idx === -1) {
            parts.push({ text: text.slice(i), match: false })
            break
        }
        if (idx > i) parts.push({ text: text.slice(i, idx), match: false })
        parts.push({ text: text.slice(idx, idx + query.length), match: true })
        i = idx + query.length
    }
    return parts
}

function buildChord(keys: Set<string>): string {
    const mods: string[] = []
    const main: string[] = []
    for (const code of keys) {
        if (code === 'ControlLeft' || code === 'ControlRight') mods.push('Ctrl')
        else if (code === 'ShiftLeft' || code === 'ShiftRight')
            mods.push('Shift')
        else if (code === 'AltLeft' || code === 'AltRight') mods.push('Alt')
        else if (code === 'MetaLeft' || code === 'MetaRight') mods.push('Meta')
        else main.push(code)
    }
    const uniqueMods = [...new Set(mods)]
    return [...uniqueMods, ...main].join('+')
}

function buildCaptureValue(keys: Set<string>): string {
    const codes = [...keys]
    if (props.singleKeyOnly) return codes.at(-1) ?? ''
    if (codes.length === 1 && isModifierCode(codes[0]!)) return codes[0]!
    return buildChord(keys)
}

function isModifierCode(code: string) {
    return MODIFIER_CODES.has(code)
}

function resetCaptureState() {
    captureActive.value = false
    pressedCaptureKeys.value.clear()
    capturedChordKeys.value.clear()
    capturedDraft.value = ''
    captureOriginalDraft.value = ''
}

function stopCapture() {
    resetCaptureState()
}

function focusCaptureButton() {
    nextTick(() => {
        const refValue = captureButtonRef.value
        const root = refValue instanceof HTMLElement ? refValue : refValue?.$el
        if (!(root instanceof HTMLElement)) return
        const element = root.matches(
            'button, [href], input, select, textarea, [tabindex]',
        )
            ? root
            : root.querySelector<HTMLElement>(
                  'button, [href], input, select, textarea, [tabindex]',
              )
        element?.focus()
    })
}

function cancelCapture() {
    draft.value = captureOriginalDraft.value
    resetCaptureState()
    focusCaptureButton()
}

function commitCapture(value: string) {
    draft.value = value
    resetCaptureState()
    nextTick(() => {
        showSuggestions.value = false
        activeIndex.value = -1
    })
    emit('pick', value)
    focusCaptureButton()
}

function selectItem(item: ActionItem) {
    stopCapture()
    draft.value = item.value
    showSuggestions.value = false
    activeIndex.value = -1
    emit('pick', item.value)
}

function closeSuggestions() {
    showSuggestions.value = false
    activeIndex.value = -1
}

function updateDraftFromInput(value: string) {
    draft.value = value
    showSuggestions.value = !!value.trim()
    if (!showSuggestions.value) activeIndex.value = -1
}

function onDocumentKeydown(event: KeyboardEvent) {
    if (!captureActive.value) return
    event.stopPropagation()
    event.preventDefault()
    if (event.key === 'Escape') {
        cancelCapture()
        return
    }
    if (!event.code) return
    pressedCaptureKeys.value.add(event.code)
    capturedChordKeys.value.add(event.code)
    capturedDraft.value = buildCaptureValue(capturedChordKeys.value)
}

function onDocumentKeyup(event: KeyboardEvent) {
    if (!captureActive.value) return
    event.stopPropagation()
    event.preventDefault()
    if (!event.code) return
    pressedCaptureKeys.value.delete(event.code)
    if (
        pressedCaptureKeys.value.size === 0 &&
        capturedChordKeys.value.size > 0
    ) {
        commitCapture(buildCaptureValue(capturedChordKeys.value))
    }
}

function getMouseButtonCode(button: number): string | null {
    switch (button) {
        case 0:
            return 'MouseLeft'
        case 1:
            return 'MouseMiddle'
        case 2:
            return 'MouseRight'
        case 3:
            return 'MouseBack'
        case 4:
            return 'MouseForward'
        default:
            return null
    }
}

function onDocumentMousedown(event: MouseEvent) {
    if (!captureActive.value) return
    const code = getMouseButtonCode(event.button)
    if (!code) return
    event.stopPropagation()
    event.preventDefault()
    pressedCaptureKeys.value.add(code)
    capturedChordKeys.value.add(code)
    capturedDraft.value = buildCaptureValue(capturedChordKeys.value)
}

function onDocumentMouseup(event: MouseEvent) {
    if (!captureActive.value) return
    const code = getMouseButtonCode(event.button)
    if (!code) return
    event.stopPropagation()
    event.preventDefault()
    pressedCaptureKeys.value.delete(code)
    if (
        pressedCaptureKeys.value.size === 0 &&
        capturedChordKeys.value.size > 0
    ) {
        commitCapture(buildCaptureValue(capturedChordKeys.value))
    }
}

function onDocumentContextmenu(event: MouseEvent) {
    if (!captureActive.value) return
    event.stopPropagation()
    event.preventDefault()
}

function toggleCapture() {
    if (captureActive.value) {
        cancelCapture()
        return
    }
    captureOriginalDraft.value = draft.value
    pressedCaptureKeys.value.clear()
    capturedChordKeys.value.clear()
    capturedDraft.value = ''
    showSuggestions.value = false
    activeIndex.value = -1
    captureActive.value = true
}

function stopOverlayPointer(event: Event) {
    event.stopPropagation()
}

function cancelCaptureFromPointer(event: Event) {
    event.stopPropagation()
    event.preventDefault()
    cancelCapture()
}

function handleInputKeydown(event: KeyboardEvent) {
    if (captureActive.value) {
        event.preventDefault()
        return
    }
    if (event.key === 'Escape') {
        closeSuggestions()
        return
    }
    if (event.key === 'ArrowDown' && props.filteredItems.length > 0) {
        event.preventDefault()
        showSuggestions.value = true
        activeIndex.value = 0
    }
}

function handleSuggestionKeydown(
    event: KeyboardEvent,
    item: ActionItem,
    index: number,
) {
    if (event.key === 'ArrowDown') {
        event.preventDefault()
        if (index < props.filteredItems.length - 1) {
            activeIndex.value = index + 1
        }
    } else if (event.key === 'ArrowUp') {
        event.preventDefault()
        if (index > 0) {
            activeIndex.value = index - 1
        } else {
            activeIndex.value = -1
        }
    } else if (event.key === 'PageDown') {
        event.preventDefault()
        activeIndex.value = Math.min(index + 5, props.filteredItems.length - 1)
    } else if (event.key === 'PageUp') {
        event.preventDefault()
        activeIndex.value = Math.max(index - 5, 0)
    } else if (event.key === 'Home') {
        event.preventDefault()
        activeIndex.value = 0
    } else if (event.key === 'End') {
        event.preventDefault()
        activeIndex.value = props.filteredItems.length - 1
    } else if (event.key === 'Enter') {
        event.preventDefault()
        selectItem(item)
    } else if (event.key === 'Escape') {
        closeSuggestions()
    }
}

function handleContainerFocusOut(event: FocusEvent) {
    const container = event.currentTarget as HTMLElement
    const related = event.relatedTarget as Node | null
    if (related && container.contains(related)) return
    if (focusoutTimer) clearTimeout(focusoutTimer)
    focusoutTimer = setTimeout(() => {
        if (!container.contains(document.activeElement)) {
            showSuggestions.value = false
            activeIndex.value = -1
        }
    }, 100)
}

function onInputFocus() {
    if (draft.value.trim()) showSuggestions.value = true
}

watch(
    () => props.filteredItems,
    (items) => {
        if (activeIndex.value >= items.length) {
            activeIndex.value = items.length > 0 ? items.length - 1 : -1
        }
    },
)

watch(
    () => props.selectionVersion,
    () => {
        closeSuggestions()
    },
)

watch(activeIndex, (idx) => {
    nextTick(() => {
        if (idx < 0) {
            inputRef.value?.focus()
            return
        }
        const btn = suggestionsRef.value?.querySelector<HTMLButtonElement>(
            `[data-suggestion-index="${idx}"]`,
        )
        btn?.focus()
        btn?.scrollIntoView({ block: 'nearest' })
    })
})

onMounted(() => {
    document.addEventListener('keydown', onDocumentKeydown, true)
    document.addEventListener('keyup', onDocumentKeyup, true)
    document.addEventListener('mousedown', onDocumentMousedown, true)
    document.addEventListener('mouseup', onDocumentMouseup, true)
    document.addEventListener('contextmenu', onDocumentContextmenu, true)
})

onBeforeUnmount(() => {
    document.removeEventListener('keydown', onDocumentKeydown, true)
    document.removeEventListener('keyup', onDocumentKeyup, true)
    document.removeEventListener('mousedown', onDocumentMousedown, true)
    document.removeEventListener('mouseup', onDocumentMouseup, true)
    document.removeEventListener('contextmenu', onDocumentContextmenu, true)
    resetCaptureState()
    if (focusoutTimer) clearTimeout(focusoutTimer)
})
</script>

<template>
    <UFormField :label="$t('picker.currentValue')">
        <div class="space-y-2">
            <div
                v-if="!isTextCategory"
                class="relative"
                @focusout="handleContainerFocusOut"
            >
                <div class="flex gap-2">
                    <InputWithClearButton
                        ref="inputRef"
                        :model-value="draft"
                        :placeholder="$t('picker.valuePh')"
                        class="w-full font-mono"
                        role="combobox"
                        aria-autocomplete="list"
                        :aria-expanded="showSuggestions"
                        :aria-controls="listboxId"
                        :aria-activedescendant="activeDescendantId"
                        @update:model-value="updateDraftFromInput"
                        @focus="onInputFocus"
                        @keydown="handleInputKeydown"
                    />
                    <UButton
                        ref="captureButtonRef"
                        icon="i-lucide-keyboard"
                        color="neutral"
                        variant="outline"
                        class="shrink-0 bg-yellow-100 text-yellow-950 shadow-sm ring-yellow-300/80 hover:bg-yellow-200 active:bg-yellow-200 focus-visible:ring-yellow-400 dark:bg-yellow-400/15 dark:text-yellow-100 dark:ring-yellow-400/45 dark:hover:bg-yellow-400/25 dark:active:bg-yellow-400/25"
                        :aria-label="$t('picker.captureKeys')"
                        :title="$t('picker.captureKeys')"
                        @click="toggleCapture"
                    >
                        {{ $t('picker.captureKeys') }}
                    </UButton>
                </div>
                <div
                    v-if="showSuggestions"
                    :id="listboxId"
                    ref="suggestionsRef"
                    role="listbox"
                    class="absolute z-20 left-0 right-0 top-full mt-1 max-h-80 overflow-y-auto rounded-md border border-(--ui-border) bg-(--ui-bg-elevated) shadow-lg p-1 space-y-0.5"
                >
                    <div
                        v-if="!filteredItems.length"
                        class="px-2.5 py-2 text-sm text-(--ui-text-muted) italic text-center"
                    >
                        {{ $t('picker.noResults') }}
                    </div>
                    <button
                        v-for="(item, index) in filteredItems"
                        :id="`${pickerId}-option-${index}`"
                        :key="item.value"
                        type="button"
                        role="option"
                        :aria-selected="draft === item.value"
                        :data-suggestion-index="index"
                        class="w-full text-left px-2.5 py-1.5 rounded-md border text-sm transition-colors hover:bg-(--ui-bg-elevated) focus:outline-none focus-visible:ring-2 focus-visible:ring-(--ui-primary) focus-visible:ring-offset-1 focus-visible:ring-offset-(--ui-bg-elevated)"
                        :class="[
                            draft === item.value
                                ? 'border-(--ui-primary) bg-(--ui-primary)/10'
                                : 'border-(--ui-border) bg-(--ui-bg)',
                            activeIndex === index
                                ? 'ring-2 ring-(--ui-primary) ring-offset-1 ring-offset-(--ui-bg-elevated)'
                                : '',
                        ]"
                        @mousedown.stop.prevent
                        @click="selectItem(item)"
                        @keydown="handleSuggestionKeydown($event, item, index)"
                    >
                        <div class="truncate font-medium">
                            <template
                                v-for="(part, pi) in highlightParts(
                                    item.label,
                                    draft.trim(),
                                )"
                                :key="`l-${pi}`"
                            >
                                <mark
                                    v-if="part.match"
                                    class="bg-(--ui-primary)/20 text-(--ui-primary) rounded px-0.5"
                                    >{{ part.text }}</mark
                                >
                                <template v-else>{{ part.text }}</template>
                            </template>
                        </div>
                        <div
                            v-if="item.hint && item.hint !== item.label"
                            class="truncate text-[11px] text-(--ui-text-muted) font-mono"
                        >
                            <template
                                v-for="(part, pi) in highlightParts(
                                    item.hint,
                                    draft.trim(),
                                )"
                                :key="`h-${pi}`"
                            >
                                <mark
                                    v-if="part.match"
                                    class="bg-(--ui-primary)/20 text-(--ui-primary) rounded px-0.5"
                                    >{{ part.text }}</mark
                                >
                                <template v-else>{{ part.text }}</template>
                            </template>
                        </div>
                    </button>
                </div>
            </div>
            <UTextarea
                v-else
                v-model="textDraft"
                :rows="5"
                :placeholder="$t('picker.textPh')"
                class="w-full font-mono"
            />
            <p v-if="isTextCategory" class="text-xs text-(--ui-text-muted)">
                {{ $t('picker.textHint') }}
            </p>
            <Teleport to="body">
                <div
                    v-if="captureActive"
                    class="fixed inset-0 z-[9999] flex items-center justify-center bg-(--ui-bg)/90 p-4"
                    data-testid="key-capture-overlay"
                    role="dialog"
                    aria-modal="true"
                    :aria-labelledby="dialogTitleId"
                    @pointerdown="stopOverlayPointer"
                    @pointerup="stopOverlayPointer"
                    @mousedown="stopOverlayPointer"
                    @mouseup="stopOverlayPointer"
                    @click="stopOverlayPointer"
                >
                    <div
                        class="w-full max-w-md rounded-lg border border-(--ui-border) bg-(--ui-bg-elevated) p-5 shadow-xl"
                        @pointerdown="stopOverlayPointer"
                        @pointerup="stopOverlayPointer"
                        @mousedown="stopOverlayPointer"
                        @mouseup="stopOverlayPointer"
                        @click="stopOverlayPointer"
                    >
                        <div class="flex items-start gap-3">
                            <div
                                class="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-(--ui-primary)/10 text-(--ui-primary)"
                            >
                                <UIcon
                                    name="i-lucide-keyboard"
                                    class="h-5 w-5"
                                />
                            </div>
                            <div class="min-w-0 flex-1">
                                <h3
                                    :id="dialogTitleId"
                                    class="text-sm font-semibold"
                                >
                                    {{ $t('picker.listeningKeys') }}
                                </h3>
                                <p class="mt-1 text-sm text-(--ui-text-muted)">
                                    {{ $t('picker.pressEscapeToStop') }}
                                </p>
                            </div>
                        </div>
                        <div
                            class="mt-5 rounded-md border border-(--ui-border) bg-(--ui-bg) px-3 py-3 font-mono text-sm"
                        >
                            {{ capturedDraft || $t('picker.valuePh') }}
                        </div>
                        <div class="mt-5 flex justify-end">
                            <UButton
                                color="neutral"
                                variant="ghost"
                                @pointerdown="cancelCaptureFromPointer"
                                @click="cancelCaptureFromPointer"
                            >
                                {{ $t('common.cancel') }}
                            </UButton>
                        </div>
                    </div>
                </div>
            </Teleport>
        </div>
    </UFormField>
</template>
