<script setup lang="ts">
import { commandActionRef, macroActionRef, systemActionRef } from '~/types/config'
import { SYSTEM_ACTIONS } from '~/utils/systemActions'
import { SYSTEM_MACROS } from '~/utils/systemMacros'
import {
  STATIC_CATEGORIES,
  type ActionItem,
  type StaticCategory,
} from '~/utils/actionCategories'

const props = defineProps<{
  keyOnly?: boolean
  spacious?: boolean
  excludedMacroId?: string
  allowMacros?: boolean
}>()

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
  if (!allCategories.value.some((c) => c.id === activeCategory.value)) {
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

function pick(item: ActionItem) {
  draft.value = item.value
  emit('pick', item.value)
}
</script>

<template>
  <div :class="props.spacious ? 'min-h-0 flex flex-1 flex-col gap-4' : 'space-y-4'">
    <UFormField :label="$t('picker.currentValue')">
      <div class="space-y-2">
        <UInput
          v-model="draft"
          :placeholder="$t('picker.valuePh')"
          class="w-full font-mono"
        />
        <p
          v-if="showChordHint"
          class="text-xs text-(--ui-text-muted)"
        >
          {{ $t('picker.chordHint') }}
        </p>
      </div>
    </UFormField>

    <div class="flex flex-wrap gap-1.5 border-b border-(--ui-border) pb-2">
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
      v-if="categoryItems.length === 0"
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
