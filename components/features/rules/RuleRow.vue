<script setup lang="ts">
import type { LayerRule } from '~/types/config'

type ModeKind = 'native' | 'none' | 'action'

defineProps<{
  rule: LayerRule
  layerOptions: Array<{ label: string, value: string }>
  defaultHoldTimeoutMs: number
  defaultDoubleTapTimeoutMs: number
}>()

defineEmits<{
  remove: [id: string]
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
  <div
    class="grid grid-cols-[1fr_1fr_1.2fr_1.2fr_1fr_auto_auto_auto] gap-3 items-start p-3 rounded-md bg-(--ui-bg-muted)"
  >
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

    <div class="pt-6">
      <UButton
        icon="i-lucide-trash-2"
        color="error"
        variant="ghost"
        square
        :aria-label="$t('rules.deleteRule')"
        @click="$emit('remove', rule.id)"
      />
    </div>
  </div>
</template>
