<script setup lang="ts">
import { ALL_KEYS } from '~/utils/keys'
import { macroActionRef } from '~/types/config'

const props = defineProps<{
  placeholder?: string
  // When true, show a "— none —" sentinel at the top of the list.
  allowEmpty?: boolean
}>()

const model = defineModel<string>({ default: '' })

const { macros, displayAction } = useMacros()

// Flat list of all known key codes + a few common action sugar strings.
// `createItem` in UInputMenu lets the user type anything arbitrary and
// press Enter to use it as-is, so this list is just a suggestions set.
const SUGGESTED_ACTIONS: string[] = [
  'Escape',
  'Enter',
  'Tab',
  'Backspace',
  'Delete',
  'Space',
  'Left',
  'Right',
  'Up',
  'Down',
  'Home',
  'End',
  'PageUp',
  'PageDown',
  'BrowserBack',
  'BrowserForward',
  'VolumeUp',
  'VolumeDown',
  'VolumeMute',
  'MediaPlayPause',
  'MediaNext',
  'MediaPrev',
]

const items = computed(() => {
  const codes = ALL_KEYS.map((k) => k.code)
  const macroItems = macros.value.map((m) => ({
    label: `▶ ${m.name || m.id}`,
    value: macroActionRef(m.id),
  }))
  // Deduplicate while keeping macros on top, then suggestions, then codes.
  const set = new Set<string>()
  const result: Array<{ label: string; value: string }> = []
  for (const it of macroItems) {
    if (!set.has(it.value)) {
      set.add(it.value)
      result.push(it)
    }
  }
  for (const v of [...SUGGESTED_ACTIONS, ...codes]) {
    if (!set.has(v)) {
      set.add(v)
      result.push({ label: v, value: v })
    }
  }
  return result
})

const displayLabel = computed(() => displayAction(model.value))
</script>

<template>
  <div class="flex items-center gap-1">
    <UInputMenu
      v-model="model"
      :items="items"
      value-key="value"
      create-item
      :placeholder="placeholder ?? 'клавиша или действие'"
      class="flex-1 min-w-0"
    />
    <span
      v-if="displayLabel && displayLabel !== model"
      class="text-[10px] text-(--ui-text-muted) italic truncate max-w-[8rem]"
      :title="displayLabel"
    >{{ displayLabel }}</span>
    <UButton
      v-if="allowEmpty && model"
      icon="i-lucide-x"
      size="xs"
      color="neutral"
      variant="ghost"
      square
      aria-label="Очистить"
      @click="model = ''"
    />
  </div>
</template>
