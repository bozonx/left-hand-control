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
}>()

const emit = defineEmits<{
  pick: [value: string]
}>()

const draft = defineModel<string>({ default: '' })

const showSuggestions = ref(false)
const inputRef = ref<InstanceType<typeof InputWithClearButton> | null>(null)
const activeIndex = ref(-1)

const captureActive = ref(false)
const capturedKeys = ref<Set<string>>(new Set())

const isTextCategory = computed(() => props.activeCategory === 'text')
const showChordHint = computed(() => props.keyOnly)
const textDraft = computed({
  get: () => parseTextAction(draft.value) ?? '',
  set: (value: string) => {
    draft.value = textActionRef(value)
  },
})

let focusoutTimer: ReturnType<typeof setTimeout> | null = null

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
    else if (code === 'ShiftLeft' || code === 'ShiftRight') mods.push('Shift')
    else if (code === 'AltLeft' || code === 'AltRight') mods.push('Alt')
    else if (code === 'MetaLeft' || code === 'MetaRight') mods.push('Meta')
    else main.push(code)
  }
  const uniqueMods = [...new Set(mods)]
  return [...uniqueMods, ...main].join('+')
}

function stopCapture() {
  captureActive.value = false
  capturedKeys.value.clear()
}

function selectItem(item: ActionItem) {
  stopCapture()
  draft.value = item.value
  showSuggestions.value = false
  activeIndex.value = -1
  emit('pick', item.value)
}

function onDocumentKeydown(event: KeyboardEvent) {
  if (!captureActive.value) return
  event.stopPropagation()
  if (event.key === 'Escape') {
    stopCapture()
    return
  }
  event.preventDefault()
  capturedKeys.value.add(event.code)
  draft.value = buildChord(capturedKeys.value)
}

function onDocumentKeyup(event: KeyboardEvent) {
  if (!captureActive.value) return
  event.stopPropagation()
  capturedKeys.value.delete(event.code)
  if (capturedKeys.value.size === 0) {
    captureActive.value = false
  }
}

function toggleCapture() {
  captureActive.value = !captureActive.value
  if (captureActive.value) {
    capturedKeys.value.clear()
  }
}

function handleInputKeydown(event: KeyboardEvent) {
  if (captureActive.value) {
    event.preventDefault()
    return
  }
  if (event.key === 'Escape') {
    showSuggestions.value = false
    activeIndex.value = -1
    return
  }
  if (event.key === 'ArrowDown' && props.filteredItems.length > 0) {
    event.preventDefault()
    showSuggestions.value = true
    activeIndex.value = 0
  }
}

function handleSuggestionKeydown(event: KeyboardEvent, item: ActionItem, index: number) {
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
    showSuggestions.value = false
    activeIndex.value = -1
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

watch(() => props.filteredItems, (items) => {
  if (activeIndex.value >= items.length) {
    activeIndex.value = items.length > 0 ? items.length - 1 : -1
  }
})

watch(activeIndex, (idx) => {
  nextTick(() => {
    if (idx < 0) {
      inputRef.value?.focus()
      return
    }
    const btn = document.querySelector(`[data-suggestion-index="${idx}"]`) as HTMLButtonElement | null
    btn?.focus()
    btn?.scrollIntoView({ block: 'nearest' })
  })
})

watch(draft, (val) => {
  showSuggestions.value = !!val.trim()
})

onMounted(() => {
  document.addEventListener('keydown', onDocumentKeydown, true)
  document.addEventListener('keyup', onDocumentKeyup, true)
})

onBeforeUnmount(() => {
  document.removeEventListener('keydown', onDocumentKeydown, true)
  document.removeEventListener('keyup', onDocumentKeyup, true)
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
            v-model="draft"
            :placeholder="$t('picker.valuePh')"
            class="w-full font-mono"
            @focus="onInputFocus"
            @keydown="handleInputKeydown"
          />
          <UButton
            :icon="captureActive ? 'i-lucide-circle-stop' : 'i-lucide-keyboard'"
            :color="captureActive ? 'error' : 'neutral'"
            :variant="captureActive ? 'solid' : 'subtle'"
            size="sm"
            class="shrink-0"
            :aria-label="$t('picker.assignKey')"
            @click="toggleCapture"
          >
            {{ captureActive ? $t('picker.listeningKeys') : $t('picker.assignKey') }}
          </UButton>
        </div>
        <div
          v-if="showSuggestions"
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
            :key="item.value"
            type="button"
            :data-suggestion-index="index"
            class="w-full text-left px-2.5 py-1.5 rounded-md border text-sm transition-colors hover:bg-(--ui-bg-elevated) focus:outline-none focus-visible:ring-2 focus-visible:ring-(--ui-primary) focus-visible:ring-offset-1 focus-visible:ring-offset-(--ui-bg-elevated)"
            :class="[
              draft === item.value
                ? 'border-(--ui-primary) bg-(--ui-primary)/10'
                : 'border-(--ui-border) bg-(--ui-bg)',
              activeIndex === index ? 'ring-2 ring-(--ui-primary) ring-offset-1 ring-offset-(--ui-bg-elevated)' : '',
            ]"
            @mousedown.stop.prevent
            @click="selectItem(item)"
            @keydown="handleSuggestionKeydown($event, item, index)"
          >
            <div class="truncate font-medium">
              <template v-for="(part, pi) in highlightParts(item.label, draft.trim())" :key="`l-${pi}`">
                <mark v-if="part.match" class="bg-(--ui-primary)/20 text-(--ui-primary) rounded px-0.5">{{ part.text }}</mark>
                <template v-else>{{ part.text }}</template>
              </template>
            </div>
            <div
              v-if="item.hint && item.hint !== item.label"
              class="truncate text-[11px] text-(--ui-text-muted) font-mono"
            >
              <template v-for="(part, pi) in highlightParts(item.hint, draft.trim())" :key="`h-${pi}`">
                <mark v-if="part.match" class="bg-(--ui-primary)/20 text-(--ui-primary) rounded px-0.5">{{ part.text }}</mark>
                <template v-else>{{ part.text }}</template>
              </template>
            </div>
          </button>
        </div>
        <div
          v-if="captureActive"
          class="absolute inset-0 z-30 flex items-center justify-center bg-(--ui-bg)/60 backdrop-blur-sm rounded-md"
        >
          <div class="text-center space-y-1">
            <UIcon name="i-lucide-keyboard" class="w-6 h-6 mx-auto text-(--ui-primary)" />
            <p class="text-sm font-medium">{{ $t('picker.listeningKeys') }}</p>
            <p class="text-xs text-(--ui-text-muted)">{{ $t('picker.pressEscapeToStop') }}</p>
          </div>
        </div>
      </div>
      <UTextarea
        v-else
        v-model="textDraft"
        :rows="5"
        :placeholder="$t('picker.textPh')"
        class="w-full font-mono"
      />
      <p
        v-if="showChordHint"
        class="text-xs text-(--ui-text-muted)"
      >
        {{ $t('picker.chordHint') }}
      </p>
      <p
        v-if="isTextCategory"
        class="text-xs text-(--ui-text-muted)"
      >
        {{ $t('picker.textHint') }}
      </p>
    </div>
  </UFormField>
</template>
