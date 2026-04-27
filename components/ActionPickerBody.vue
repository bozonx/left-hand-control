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
const inputFocused = ref(false)

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

function onInputFocus() {
  inputFocused.value = true
  if (draft.value.trim()) showSuggestions.value = true
}

function onInputBlur() {
  inputFocused.value = false
  setTimeout(() => {
    if (!inputFocused.value) showSuggestions.value = false
  }, 120)
}

function onInputKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    showSuggestions.value = false
  }
}

watch(draft, (val) => {
  if (val.trim() && inputFocused.value) {
    showSuggestions.value = true
  } else if (!val.trim()) {
    showSuggestions.value = false
  }
})

function pick(item: ActionItem) {
  draft.value = item.value
  emit('pick', item.value)
}
</script>

<template>
  <div :class="props.spacious ? 'min-h-0 flex flex-1 flex-col gap-4' : 'space-y-4'">
    <UFormField :label="$t('picker.currentValue')">
      <div class="space-y-2">
        <div v-if="!isTextCategory" class="relative">
          <InputWithClearButton
            v-model="draft"
            :placeholder="$t('picker.valuePh')"
            class="w-full font-mono"
            @focus="onInputFocus"
            @blur="onInputBlur"
            @keydown="onInputKeydown"
          />
          <div
            v-if="showSuggestions && filteredItems.length"
            class="absolute z-20 left-0 right-0 top-full mt-1 max-h-80 overflow-y-auto rounded-md border border-(--ui-border) bg-(--ui-bg-elevated) shadow-lg p-1 space-y-0.5"
          >
            <button
              v-for="item in filteredItems"
              :key="item.value"
              type="button"
              class="w-full text-left px-2.5 py-1.5 rounded-md border text-sm transition-colors hover:bg-(--ui-bg-elevated)"
              :class="draft === item.value
                ? 'border-(--ui-primary) bg-(--ui-primary)/10'
                : 'border-(--ui-border) bg-(--ui-bg)'"
              @mousedown.stop.prevent
              @click="pick(item); showSuggestions = false"
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
