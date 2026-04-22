<script setup lang="ts">
import type { LayerRule } from '~/types/config'

type ModeKind = 'native' | 'none' | 'action'

defineProps<{
  rule: LayerRule
  layerOptions: Array<{ label: string, value: string }>
  defaultHoldTimeoutMs: number
  defaultDoubleTapTimeoutMs: number
  isFirst?: boolean
  isLast?: boolean
}>()

defineEmits<{
  remove: [id: string]
  moveUp: [id: string]
  moveDown: [id: string]
  createLayer: [ruleId: string]
}>()

function modeOf(value: string | null | undefined): ModeKind {
  if (value === null) return 'none'
  if (value && value.length) return 'action'
  return 'native'
}

function setMode(
  rule: LayerRule,
  field: 'tapAction' | 'holdAction',
  mode: ModeKind,
) {
  rule[field] = mode === 'native' ? '' : mode === 'none' ? null : rule[field] || ''
}

function actionValue(value: string | null): string {
  return typeof value === 'string' ? value : ''
}
</script>

<template>
  <div class="relative p-4 rounded-xl border border-(--ui-border) bg-(--ui-bg-muted)/40 flex gap-6">
    <!-- Left Column: Main Parameters (flex-1) -->
    <div class="flex-1 flex flex-col gap-5">
      <!-- Top Row: Key and Layer -->
      <div class="grid grid-cols-2 gap-4">
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('rules.keyLabel')"
              :hint="$t('rules.keyHint')"
            />
          </template>
          <ActionPickerModal
            v-model="rule.key"
            key-only
            :placeholder="$t('rules.keyPh')"
          />
        </UFormField>

        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('rules.layerLabel')"
              :hint="$t('rules.layerHint')"
            />
          </template>
          <div class="flex gap-1">
            <USelectMenu
              v-model="rule.layerId"
              :items="layerOptions"
              value-key="value"
              :placeholder="$t('common.none')"
              class="flex-1 min-w-0"
            />
            <UButton
              v-if="rule.layerId"
              icon="i-lucide-x"
              variant="ghost"
              color="neutral"
              square
              :aria-label="$t('rules.clearLayer')"
              @click="rule.layerId = ''"
            />
            <UButton
              icon="i-lucide-plus"
              variant="outline"
              color="neutral"
              square
              :aria-label="$t('rules.createLayer')"
              @click="$emit('createLayer', rule.id)"
            />
          </div>
        </UFormField>
      </div>

      <div class="h-px bg-(--ui-border) w-full"></div>

      <!-- Actions Row -->
      <div class="grid grid-cols-3 gap-4">
        <!-- Tap -->
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('rules.tapLabel')"
              :hint="$t('rules.tapHint')"
            />
          </template>
          <div class="space-y-1.5">
            <USelectMenu
              :model-value="modeOf(rule.tapAction)"
              :items="[
                { label: $t('rules.modeNative'), value: 'native' },
                { label: $t('rules.modeNone'), value: 'none' },
                { label: $t('rules.modeAction'), value: 'action' },
              ]"
              value-key="value"
              class="w-full"
              @update:model-value="(mode: ModeKind) => setMode(rule, 'tapAction', mode)"
            />
            <ActionPickerModal
              v-if="modeOf(rule.tapAction) === 'action'"
              :model-value="actionValue(rule.tapAction)"
              allow-empty
              :placeholder="$t('rules.tapPh')"
              @update:model-value="(value: string) => rule.tapAction = value"
            />
          </div>
        </UFormField>

        <!-- Hold -->
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('rules.holdActionLabel')"
              :hint="$t('rules.holdActionHint')"
            />
          </template>
          <div class="space-y-1.5">
            <USelectMenu
              :model-value="modeOf(rule.holdAction)"
              :items="[
                { label: $t('rules.modeNative'), value: 'native' },
                { label: $t('rules.modeNone'), value: 'none' },
                { label: $t('rules.modeAction'), value: 'action' },
              ]"
              value-key="value"
              class="w-full"
              @update:model-value="(mode: ModeKind) => setMode(rule, 'holdAction', mode)"
            />
            <ActionPickerModal
              v-if="modeOf(rule.holdAction) === 'action'"
              :model-value="actionValue(rule.holdAction)"
              key-only
              allow-empty
              :placeholder="$t('rules.holdActionPh')"
              @update:model-value="(value: string) => rule.holdAction = value"
            />
          </div>
        </UFormField>

        <!-- Double Tap -->
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('rules.doubleTapLabel')"
              :hint="$t('rules.doubleTapHint')"
            />
          </template>
          <ActionPickerModal
            v-model="rule.doubleTapAction"
            allow-empty
            :placeholder="$t('rules.doubleTapPh')"
          />
        </UFormField>
      </div>
    </div>

    <!-- Vertical Separator -->
    <div class="w-px bg-(--ui-border) self-stretch"></div>

    <!-- Right Column: Meta & Timing -->
    <div class="w-52 flex flex-col gap-4">
      <div class="flex items-center justify-between">
        <div class="flex gap-1">
          <UButton
            icon="i-lucide-arrow-up"
            variant="ghost"
            color="neutral"
            size="sm"
            square
            :disabled="isFirst"
            @click="$emit('moveUp', rule.id)"
          />
          <UButton
            icon="i-lucide-arrow-down"
            variant="ghost"
            color="neutral"
            size="sm"
            square
            :disabled="isLast"
            @click="$emit('moveDown', rule.id)"
          />
        </div>

        <UButton
          icon="i-lucide-trash-2"
          color="neutral"
          variant="ghost"
          size="sm"
          square
          :aria-label="$t('rules.deleteRule')"
          @click="$emit('remove', rule.id)"
        />
      </div>

      <div class="flex flex-col gap-4">
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('rules.holdLabel')"
              :hint="$t('rules.holdHint')"
            />
          </template>
          <OverridableNumberField
            v-model="rule.holdTimeoutMs"
            :default-value="defaultHoldTimeoutMs"
            :suffix="$t('common.ms')"
          />
        </UFormField>

        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('rules.doubleTapWindowLabel')"
              :hint="$t('rules.doubleTapWindowHint')"
            />
          </template>
          <OverridableNumberField
            v-model="rule.doubleTapTimeoutMs"
            :default-value="defaultDoubleTapTimeoutMs"
            :suffix="$t('common.ms')"
          />
        </UFormField>
      </div>
    </div>
  </div>
</template>
