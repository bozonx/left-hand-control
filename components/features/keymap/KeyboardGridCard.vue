<script setup lang="ts">
import { LEFT_HAND_ROWS, RIGHT_HAND_ROWS } from '~/utils/keys'
import type { LayerKeymap } from '~/types/config'

defineProps<{
  currentKeymap: LayerKeymap
}>()

defineEmits<{
  edit: [code: string, label: string]
}>()
</script>

<template>
  <UCard>
    <template #header>
      <h2 class="text-sm font-semibold">{{ $t('keymap.keyboardTitle') }}</h2>
    </template>
    <div class="grid grid-cols-1 xl:grid-cols-2 gap-6">
      <section>
        <div
          class="text-xs uppercase tracking-wide text-(--ui-text-muted) mb-2"
        >
          {{ $t('keymap.leftHand') }}
        </div>
        <div class="space-y-1.5">
          <div
            v-for="(kbRow, i) in LEFT_HAND_ROWS"
            :key="`l-${i}`"
            class="grid grid-cols-7 gap-1.5"
          >
            <KeyCap
              v-for="keyDef in kbRow"
              :key="keyDef.code"
              :label="keyDef.label"
              :action="currentKeymap.keys[keyDef.code]"
              @edit="$emit('edit', keyDef.code, keyDef.label)"
            />
          </div>
        </div>
      </section>
      <section>
        <div
          class="text-xs uppercase tracking-wide text-(--ui-text-muted) mb-2"
        >
          {{ $t('keymap.rightHand') }}
        </div>
        <div class="space-y-1.5">
          <div
            v-for="(kbRow, i) in RIGHT_HAND_ROWS"
            :key="`r-${i}`"
            class="grid grid-cols-8 gap-1.5"
          >
            <KeyCap
              v-for="keyDef in kbRow"
              :key="keyDef.code"
              :label="keyDef.label"
              :action="currentKeymap.keys[keyDef.code]"
              @edit="$emit('edit', keyDef.code, keyDef.label)"
            />
          </div>
        </div>
      </section>
    </div>
  </UCard>
</template>
