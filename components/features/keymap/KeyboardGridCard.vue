<script setup lang="ts">
import { LEFT_HAND_ROWS, RIGHT_HAND_ROWS, keyLabel, type KeyLabelMode } from '~/utils/keys'
import type { LayerKeymap } from '~/types/config'
import AppTooltip from '~/components/shared/AppTooltip.vue'

const props = defineProps<{
  currentKeymap: LayerKeymap
  keyLabelMode: KeyLabelMode
}>()

const emit = defineEmits<{
  edit: [code: string, label: string]
  'update:keyLabelMode': [value: KeyLabelMode]
  clear: []
}>()
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <div class="flex items-center gap-1.5">
          <h2 class="text-sm font-semibold">{{ $t('keymap.keyboardTitle') }}</h2>
          <AppTooltip :text="$t('keymap.keyboardInfo')" align="start" toggle-on-click>
            <UButton
              icon="i-lucide-info"
              color="neutral"
              variant="ghost"
              size="xs"
              square
              :aria-label="$t('keymap.keyboardInfo')"
            />
          </AppTooltip>
        </div>
        <div class="flex items-center gap-2">
          <AppTooltip :text="$t('keymap.clearKeyboardTooltip')">
            <UButton
              size="sm"
              color="neutral"
              variant="ghost"
              icon="i-lucide-eraser"
              @click="emit('clear')"
            >
              {{ $t('common.clear') }}
            </UButton>
          </AppTooltip>
          <UButtonGroup size="sm" class="bg-(--ui-bg-muted)/50 p-0.5 rounded-md">
            <UButton
              :variant="keyLabelMode === 'label' ? 'solid' : 'ghost'"
              color="neutral"
              @click="emit('update:keyLabelMode', 'label')"
            >
              {{ $t('keymap.keyViewLabels') }}
            </UButton>
            <UButton
              :variant="keyLabelMode === 'code' ? 'solid' : 'ghost'"
              color="neutral"
              @click="emit('update:keyLabelMode', 'code')"
            >
              {{ $t('keymap.keyViewCodes') }}
            </UButton>
            <UButton
              :variant="keyLabelMode === 'numeric' ? 'solid' : 'ghost'"
              color="neutral"
              @click="emit('update:keyLabelMode', 'numeric')"
            >
              {{ $t('keymap.keyViewNumeric') }}
            </UButton>
          </UButtonGroup>
        </div>
      </div>
    </template>
    <div class="grid grid-cols-1 xl:grid-cols-2 gap-6">
      <section :aria-label="$t('keymap.leftHand')">
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
              v-for="(keyDef, j) in kbRow"
              :key="keyDef.code"
              :class="{ 'col-start-3': i === LEFT_HAND_ROWS.length - 1 && j === 0 }"
              :label="keyLabel(keyDef.code, props.keyLabelMode)"
              :action="currentKeymap.keys[keyDef.code]"
              @edit="emit('edit', keyDef.code, keyLabel(keyDef.code, props.keyLabelMode))"
            />
          </div>
        </div>
      </section>
      <section :aria-label="$t('keymap.rightHand')">
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
