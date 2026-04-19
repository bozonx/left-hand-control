<script setup lang="ts">
import { ALL_KEYS } from '~/utils/keys'

const props = defineProps<{
  placeholder?: string
  // When true, show a "— none —" sentinel at the top of the list.
  allowEmpty?: boolean
}>()

const model = defineModel<string>({ default: '' })

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
  // Deduplicate while keeping suggestions first.
  const set = new Set<string>()
  const result: string[] = []
  for (const v of [...SUGGESTED_ACTIONS, ...codes]) {
    if (!set.has(v)) {
      set.add(v)
      result.push(v)
    }
  }
  return result
})
</script>

<template>
  <div class="flex items-center gap-1">
    <UInputMenu
      v-model="model"
      :items="items"
      create-item
      :placeholder="placeholder ?? 'клавиша или действие'"
      class="flex-1 min-w-0"
    />
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
