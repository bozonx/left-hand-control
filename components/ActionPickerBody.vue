<script setup lang="ts">
import InputWithClearButton from '~/components/shared/InputWithClearButton.vue'
import { commandActionRef, macroActionRef, parseTextAction, systemActionRef, textActionRef } from '~/types/config'
import { SYSTEM_ACTIONS } from '~/utils/systemActions'
import { SYSTEM_MACROS } from '~/utils/systemMacros'
import {
  STATIC_CATEGORIES,
  type ActionItem,
  type StaticCategory,
} from '~/utils/actionCategories'

interface TextPart {
  text: string
  match: boolean
}

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

const props = withDefaults(defineProps<{
  keyOnly?: boolean
  spacious?: boolean
  excludedMacroId?: string
  allowMacros?: boolean
}>(), {
  allowMacros: true,
})

const emit = defineEmits<{
  pick: [value: string]
}>()

const draft = defineModel<string>({ default: '' })

const { macros } = useMacros()
const { commands } = useCommands()
const { t } = useI18n()

const dynamicCategories = computed<StaticCategory[]>(() => {
  // User macros shadow system macros with the same id — hide duplicates
  // from the system list to avoid confusion.
  const userMacros = macros.value.filter((m) => m.id !== props.excludedMacroId)
  const userIds = new Set(userMacros.map((m) => m.id))
  return [
    ...(
      commands.value.length === 0
        ? []
        : [
            {
              id: 'commands',
              labelKey: 'categories.commands',
              icon: 'i-lucide-terminal',
              items: commands.value.map((command) => ({
                label: command.name || command.id,
                value: commandActionRef(command.id),
                hint: command.id,
              })),
            },
          ]
    ),
    ...(
      props.allowMacros === false
        ? []
        : [
            {
              id: 'macros',
              labelKey: 'categories.macros',
              icon: 'i-lucide-zap',
              items: userMacros.map((m) => ({
                label: m.name || m.id,
                value: macroActionRef(m.id),
                hint: m.id,
              })),
            },
            {
              id: 'system-macros',
              labelKey: 'categories.systemMacros',
              icon: 'i-lucide-cpu',
              items: SYSTEM_MACROS.filter((m) => (
                !userIds.has(m.id) && m.id !== props.excludedMacroId
              )).map((m) => ({
                label: m.name,
                value: macroActionRef(m.id),
                hint: m.id,
              })),
            },
          ]
    ),
    {
      id: 'system',
      labelKey: 'categories.system',
      icon: 'i-lucide-settings-2',
      items: SYSTEM_ACTIONS.map((a) => ({
        label: t(a.nameKey, a.nameParams ?? {}),
        value: systemActionRef(a.id),
        hint: a.id,
      })),
    },
  ]
})

const allCategories = computed<StaticCategory[]>(() =>
  props.keyOnly
    ? STATIC_CATEGORIES
    : [...dynamicCategories.value, ...STATIC_CATEGORIES],
)

const activeCategory = ref<string>(allCategories.value[0]?.id ?? 'special')

watchEffect(() => {
  if (
    activeCategory.value !== 'text'
    && !allCategories.value.some((c) => c.id === activeCategory.value)
  ) {
    activeCategory.value = allCategories.value[0]?.id ?? 'special'
  }
})

const categoryItems = computed(() => {
  const cat = allCategories.value.find((c) => c.id === activeCategory.value)
  return cat?.items ?? []
})

const showPhysicalKeyHint = computed(() =>
  ['lettersSymbols'].includes(activeCategory.value),
)

const showChordHint = computed(() => props.keyOnly)
const isTextCategory = computed(() => activeCategory.value === 'text')
const textDraft = computed({
  get: () => parseTextAction(draft.value) ?? '',
  set: (value: string) => {
    draft.value = textActionRef(value)
  },
})

const listGridClass = computed(() =>
  ['commands', 'macros', 'system-macros', 'system'].includes(activeCategory.value)
    ? [
        'grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-1.5 overflow-y-auto pr-1',
        props.spacious ? 'min-h-0 flex-1 content-start' : 'max-h-80',
      ].join(' ')
    : [
        'grid grid-cols-2 sm:grid-cols-4 xl:grid-cols-6 gap-1.5 overflow-y-auto pr-1',
        props.spacious ? 'min-h-0 flex-1 content-start' : 'max-h-80',
      ].join(' '),
)

const showSuggestions = ref(false)
const inputRef = ref<InstanceType<typeof InputWithClearButton> | null>(null)
const activeIndex = ref(-1)

const captureActive = ref(false)
const capturedKeys = ref<Set<string>>(new Set())

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

function stopCapture() {
  captureActive.value = false
  capturedKeys.value.clear()
}

onMounted(() => {
  document.addEventListener('keydown', onDocumentKeydown, true)
  document.addEventListener('keyup', onDocumentKeyup, true)
})

onBeforeUnmount(() => {
  document.removeEventListener('keydown', onDocumentKeydown, true)
  document.removeEventListener('keyup', onDocumentKeyup, true)
})

const filteredItems = computed(() => {
  const query = draft.value.trim().toLowerCase()
  if (!query) return []
  const allItems: ActionItem[] = []
  for (const cat of allCategories.value) {
    for (const item of cat.items) {
      const searchable = (item.hint || item.value).toLowerCase()
      if (searchable.includes(query)) {
        allItems.push(item)
      }
    }
  }
  return allItems
})

watch(filteredItems, (items) => {
  if (activeIndex.value >= items.length) {
    activeIndex.value = items.length > 0 ? items.length - 1 : -1
  }
})

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
  if (event.key === 'ArrowDown' && filteredItems.value.length > 0) {
    event.preventDefault()
    showSuggestions.value = true
    activeIndex.value = 0
  }
}

function handleSuggestionKeydown(event: KeyboardEvent, item: ActionItem, index: number) {
  if (event.key === 'ArrowDown') {
    event.preventDefault()
    if (index < filteredItems.value.length - 1) {
      activeIndex.value = index + 1
    }
  } else if (event.key === 'ArrowUp') {
    event.preventDefault()
    if (index > 0) {
      activeIndex.value = index - 1
    } else {
      activeIndex.value = -1
    }
  } else if (event.key === 'Enter') {
    event.preventDefault()
    pick(item)
    showSuggestions.value = false
    activeIndex.value = -1
  } else if (event.key === 'Escape') {
    showSuggestions.value = false
    activeIndex.value = -1
  }
}

function handleContainerFocusOut(event: FocusEvent) {
  const container = event.currentTarget as HTMLElement
  const related = event.relatedTarget as HTMLElement | null
  if (!container.contains(related)) {
    showSuggestions.value = false
    activeIndex.value = -1
  }
}

watch(activeIndex, (idx) => {
  nextTick(() => {
    if (idx < 0) {
      inputRef.value?.focus()
      return
    }
    const btn = document.querySelector(`[data-suggestion-index="${idx}"]`) as HTMLButtonElement | null
    btn?.focus()
  })
})

function onInputFocus() {
  if (draft.value.trim()) showSuggestions.value = true
}

watch(draft, (val) => {
  if (val.trim()) {
    showSuggestions.value = true
  } else {
    showSuggestions.value = false
  }
})

function pick(item: ActionItem) {
  stopCapture()
  draft.value = item.value
  emit('pick', item.value)
}
</script>

<template>
  <div :class="props.spacious ? 'min-h-0 flex flex-1 flex-col gap-4' : 'space-y-4'">
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
            v-if="showSuggestions && filteredItems.length"
            class="absolute z-20 left-0 right-0 top-full mt-1 max-h-80 overflow-y-auto rounded-md border border-(--ui-border) bg-(--ui-bg-elevated) shadow-lg p-1 space-y-0.5"
          >
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
              @click="pick(item); showSuggestions = false; activeIndex = -1"
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

    <div class="flex flex-wrap gap-1.5 border-b border-(--ui-border) pb-2">
      <UButton
        v-if="!props.keyOnly"
        icon="i-lucide-text-cursor-input"
        size="xs"
        :color="activeCategory === 'text' ? 'primary' : 'neutral'"
        :variant="activeCategory === 'text' ? 'soft' : 'ghost'"
        @click="activeCategory = 'text'"
      >
        {{ $t('categories.text') }}
      </UButton>
      <UButton
        v-for="cat in allCategories"
        :key="cat.id"
        :icon="cat.icon"
        size="xs"
        :color="activeCategory === cat.id ? 'primary' : 'neutral'"
        :variant="activeCategory === cat.id ? 'soft' : 'ghost'"
        @click="activeCategory = cat.id"
      >
        {{ $t(cat.labelKey) }}
        <UBadge
          v-if="cat.items.length"
          size="sm"
          variant="subtle"
          color="neutral"
          class="ml-1"
        >
          {{ cat.items.length }}
        </UBadge>
      </UButton>
    </div>

    <div
      v-if="isTextCategory"
      class="rounded-lg border border-(--ui-border) bg-(--ui-bg-elevated)/40 p-4"
    >
      <div class="space-y-3">
        <p class="text-sm text-(--ui-text-muted)">{{ $t('picker.textTabBody') }}</p>
        <div class="rounded-md border border-(--ui-border) bg-(--ui-bg) px-3 py-2 font-mono text-sm">
          {{ draft || textActionRef('') }}
        </div>
      </div>
    </div>
    <div
      v-else-if="categoryItems.length === 0"
      class="text-sm text-(--ui-text-muted) italic px-1 py-6 text-center"
    >
      {{ $t('picker.emptyCategory') }}
    </div>
    <div
      v-else
      :class="listGridClass"
    >
      <button
        v-for="item in categoryItems"
        :key="item.value"
        type="button"
        class="text-left px-2.5 py-1.5 rounded-md border text-sm transition-colors hover:bg-(--ui-bg-elevated)"
        :class="draft === item.value
          ? 'border-(--ui-primary) bg-(--ui-primary)/10'
          : 'border-(--ui-border) bg-(--ui-bg)'"
        @click="pick(item)"
      >
        <div class="truncate font-medium">{{ item.label }}</div>
        <div
          v-if="item.hint && item.hint !== item.label"
          class="truncate text-[11px] text-(--ui-text-muted) font-mono"
        >
          {{ item.hint }}
        </div>
      </button>
    </div>

    <p
      v-if="showPhysicalKeyHint"
      class="text-xs text-(--ui-text-muted)"
    >
      {{ $t('picker.physicalKeyHint') }}
    </p>
  </div>
</template>
