<script setup lang="ts">
import { LEFT_HAND_ROWS, RIGHT_HAND_ROWS, keyLabel, type KeyLabelMode } from '~/utils/keys'
import type { LayerKeymap } from '~/types/config'

const props = defineProps<{
  currentKeymap: LayerKeymap
  keyLabelMode: KeyLabelMode
}>()

const emit = defineEmits<{
  edit: [code: string, label: string]
  'update:keyLabelMode': [value: KeyLabelMode]
}>()
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <h2 class="text-sm font-semibold">{{ $t('keymap.keyboardTitle') }}</h2>
        <div class="flex items-center rounded-md border border-(--ui-border) bg-(--ui-bg)">
          <UButton
            size="sm"
            color="neutral"
            :variant="keyLabelMode === 'label' ? 'soft' : 'ghost'"
            @click="emit('update:keyLabelMode', 'label')"
          >
            {{ $t('keymap.keyViewLabels') }}
          </UButton>
          <UButton
            size="sm"
            color="neutral"
            :variant="keyLabelMode === 'code' ? 'soft' : 'ghost'"
            @click="emit('update:keyLabelMode', 'code')"
          >
            {{ $t('keymap.keyViewCodes') }}
          </UButton>
          <UButton
            size="sm"
            color="neutral"
            :variant="keyLabelMode === 'numeric' ? 'soft' : 'ghost'"
            @click="emit('update:keyLabelMode', 'numeric')"
          >
            {{ $t('keymap.keyViewNumeric') }}
          </UButton>
        </div>
      </div>
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
            class="grid grid-cols-6 gap-1.5"
          >
            <KeyCap
              v-for="keyDef in kbRow"
              :key="keyDef.code"
              :label="keyLabel(keyDef.code, props.keyLabelMode)"
              :action="currentKeymap.keys[keyDef.code]"
              @edit="emit('edit', keyDef.code, keyLabel(keyDef.code, props.keyLabelMode))"
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
              :label="keyLabel(keyDef.code, props.keyLabelMode)"
              :action="currentKeymap.keys[keyDef.code]"
              @edit="emit('edit', keyDef.code, keyLabel(keyDef.code, props.keyLabelMode))"
            />
          </div>
        </div>
      </section>
    </div>
  </UCard>
</template>
