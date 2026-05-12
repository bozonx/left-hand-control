<script setup lang="ts">
import type { WindowResizeDirection } from '~/composables/useWindowControls'

// With native decorations disabled the window loses its resize border, so we
// recreate it: thin invisible strips along the edges/corners that forward a
// drag to the compositor via Tauri's `startResizeDragging`. They are sized to
// span the transparent shadow gutter `AppShell` leaves around the panel.

const { available, isMaximized, startResizeDragging } = useWindowControls()

const handles: { dir: WindowResizeDirection, class: string }[] = [
  { dir: 'North', class: 'top-0 left-4 right-4 h-3.5 cursor-ns-resize' },
  { dir: 'South', class: 'bottom-0 left-4 right-4 h-3.5 cursor-ns-resize' },
  { dir: 'West', class: 'left-0 top-4 bottom-4 w-3.5 cursor-ew-resize' },
  { dir: 'East', class: 'right-0 top-4 bottom-4 w-3.5 cursor-ew-resize' },
  { dir: 'NorthWest', class: 'top-0 left-0 size-4 cursor-nwse-resize' },
  { dir: 'NorthEast', class: 'top-0 right-0 size-4 cursor-nesw-resize' },
  { dir: 'SouthWest', class: 'bottom-0 left-0 size-4 cursor-nesw-resize' },
  { dir: 'SouthEast', class: 'bottom-0 right-0 size-4 cursor-nwse-resize' },
]

function onPointerDown(dir: WindowResizeDirection, event: MouseEvent) {
  if (event.button !== 0) return
  event.preventDefault()
  void startResizeDragging(dir)
}
</script>

<template>
  <div
    v-if="available && !isMaximized"
    class="pointer-events-none fixed inset-0 z-50"
    aria-hidden="true"
  >
    <div
      v-for="handle in handles"
      :key="handle.dir"
      class="pointer-events-auto absolute"
      :class="handle.class"
      @mousedown="onPointerDown(handle.dir, $event)"
    />
  </div>
</template>
