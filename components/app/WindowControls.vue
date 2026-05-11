<script setup lang="ts">
import type { WindowButton } from '~/composables/useWindowControls'

const props = defineProps<{
  side: 'left' | 'right'
}>()

const { layout, isMaximized, available, minimize, toggleMaximize, closeToTray }
  = useWindowControls()
const { t } = useI18n()

const buttons = computed<WindowButton[]>(() =>
  props.side === 'left' ? layout.value.left : layout.value.right,
)

function iconFor(button: WindowButton): string {
  switch (button) {
    case 'minimize':
      return 'i-lucide-minus'
    case 'maximize':
      return isMaximized.value ? 'i-lucide-copy' : 'i-lucide-square'
    case 'close':
      return 'i-lucide-x'
  }
}

function labelFor(button: WindowButton): string {
  switch (button) {
    case 'minimize':
      return t('app.windowMinimize')
    case 'maximize':
      return isMaximized.value ? t('app.windowRestore') : t('app.windowMaximize')
    case 'close':
      return t('app.windowClose')
  }
}

function activate(button: WindowButton) {
  if (button === 'minimize') void minimize()
  else if (button === 'maximize') void toggleMaximize()
  else void closeToTray()
}
</script>

<template>
  <div
    v-if="available && buttons.length"
    class="flex items-center shrink-0"
    :class="side === 'left' ? '-ml-2 mr-1' : 'ml-1 -mr-2'"
  >
    <UButton
      v-for="button in buttons"
      :key="button"
      color="neutral"
      variant="ghost"
      size="sm"
      :square="true"
      :icon="iconFor(button)"
      :aria-label="labelFor(button)"
      :title="labelFor(button)"
      :class="
        button === 'close'
          ? 'text-(--ui-text-muted) hover:text-(--ui-error) hover:bg-(--ui-error)/10'
          : 'text-(--ui-text-muted) hover:text-(--ui-text)'
      "
      @click="activate(button)"
    />
  </div>
</template>
