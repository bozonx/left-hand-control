<script setup lang="ts">
import { ref } from 'vue'
import type { LayerRule } from '~/types/config'
import SettingTimeoutField from '~/components/SettingTimeoutField.vue'
import ResettableSelectMenu from '~/components/shared/ResettableSelectMenu.vue'
import RuleActionField from '~/components/features/rules/RuleActionField.vue'
import AppTooltip from '~/components/shared/AppTooltip.vue'
import RuleConditionsModal from './RuleConditionsModal.vue'

defineOptions({
  inheritAttrs: false
})

defineProps<{
  layerOptions: Array<{ label: string, value: string }>
  defaultHoldTimeoutMs: number
  defaultDoubleTapTimeoutMs: number
  isFirst?: boolean
  isLast?: boolean
  isNew?: boolean
  keyError?: string
}>()

const rule = defineModel<LayerRule>('rule', { required: true })

const emit = defineEmits<{
  remove: [id: string]
  moveUp: [id: string]
  moveDown: [id: string]
  createLayer: [ruleId: string]
  keySelected: [id: string]
}>()

const isConditionsOpen = ref(false)

function setRuleKey(value: string | null) {
  rule.value.key = value ?? ''
  if (value) {
    emit('keySelected', rule.value.id)
  }
}

function updateHoldAction(value: string | null) {
  rule.value.holdAction = value
  if (!rule.value.holdAction) {
    rule.value.isolate = ''
  }
}
</script>

<template>
  <div
    class="relative flex gap-6 rounded-xl border p-4 transition-all duration-150 group group/rule hover:shadow-lg"
    :class="[
      rule.enabled === false ? 'opacity-50 grayscale-[30%]' : '',
      isNew
        ? 'border-(--ui-info)/60 bg-(--ui-info)/8 ring-1 ring-(--ui-info)/20 hover:bg-(--ui-info)/12 hover:border-(--ui-info)/70 hover:shadow-(--ui-info)/10'
        : 'border-(--ui-border) bg-(--ui-bg-muted)/40 hover:bg-(--ui-bg-muted)/60 hover:border-(--ui-primary)/50 hover:shadow-(--ui-primary)/5'
    ]"
  >
    <div class="flex-1 flex flex-col gap-5">
      <div class="grid grid-cols-3 gap-4">
        <UFormField :error="keyError">
          <template #label>
            <FieldLabel
              :label="$t('rules.keyLabel')"
              :hint="$t('rules.keyHint')"
              hint-visible-on="group-hover-rule"
            />
          </template>
          <ActionPickerModal
            v-model="rule.key"
            key-only
            single-key-only
            :invalid="!!keyError"
            :placeholder="$t('rules.keyPh')"
            @update:model-value="setRuleKey"
          />
        </UFormField>

        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('rules.conditionsLabel')"
              :hint="$t('rules.conditionsHint')"
              hint-visible-on="group-hover-rule"
            />
          </template>
          <UButton
            v-if="!((rule.conditionGameMode && rule.conditionGameMode !== 'ignore') || rule.conditionLayouts?.length || rule.conditionAppsWhitelist?.length || rule.conditionAppsBlacklist?.length)"
            color="neutral"
            variant="ghost"
            class="w-full h-8 px-2.5 justify-start border border-dashed border-(--ui-border) text-(--ui-text-muted) hover:text-(--ui-text) hover:border-(--ui-border-accent) hover:bg-(--ui-bg-elevated)/50 font-normal"
            @click="isConditionsOpen = true"
          >
            <span class="truncate">{{ $t('common.notSet') }}</span>
          </UButton>
          <UButton
            v-else
            color="neutral"
            variant="soft"
            class="w-full justify-between"
            trailing-icon="i-lucide-chevron-down"
            @click="isConditionsOpen = true"
          >
            <span class="truncate">{{ $t('rules.conditionsBtn') }}</span>
          </UButton>
          <div
            v-if="
              (rule.conditionGameMode && rule.conditionGameMode !== 'ignore')
                || rule.conditionLayouts?.length
                || rule.conditionAppsWhitelist?.length
                || rule.conditionAppsBlacklist?.length
            "
            class="flex flex-wrap gap-1 mt-1.5"
          >
            <UBadge
              v-if="rule.conditionGameMode && rule.conditionGameMode !== 'ignore'"
              size="xs"
              variant="subtle"
              color="primary"
            >
              {{ rule.conditionGameMode === 'on' ? $t('rules.gameModeOn') : $t('rules.gameModeOff') }}
            </UBadge>
            <UBadge
              v-if="rule.conditionLayouts?.length"
              size="xs"
              variant="subtle"
              color="neutral"
            >
              {{ rule.conditionLayouts.length }} {{ $t('rules.layoutsLabel') }}
            </UBadge>
            <UBadge
              v-if="rule.conditionAppsWhitelist?.length"
              size="xs"
              variant="subtle"
              color="success"
            >
              {{ $t('rules.appsWhitelistCount', { count: rule.conditionAppsWhitelist.length }) }}
            </UBadge>
            <UBadge
              v-if="rule.conditionAppsBlacklist?.length"
              size="xs"
              variant="subtle"
              color="error"
            >
              {{ $t('rules.appsBlacklistCount', { count: rule.conditionAppsBlacklist.length }) }}
            </UBadge>
          </div>
        </UFormField>

        <UFormField>
          <template #label>
            <span class="flex w-full items-center justify-between gap-2">
              <FieldLabel
                :label="$t('rules.layerLabel')"
                :hint="$t('rules.layerHint')"
                hint-visible-on="group-hover-rule"
              />
              <ULink
                class="ml-auto shrink-0 text-right text-xs text-(--ui-text-muted) opacity-0 group-hover/rule:opacity-100 hover:text-(--ui-primary) transition-all duration-200 cursor-pointer"
                @click="$emit('createLayer', rule.id)"
              >
                {{ $t('rules.createLayer') }}
              </ULink>
            </span>
          </template>
          <ResettableSelectMenu
            :model-value="rule.layerId"
            :items="layerOptions"
            value-key="value"
            :placeholder="$t('rules.layerPh')"
            reset-value=""
            :reset-aria-label="$t('rules.clearLayer')"
            empty-item-value="__none__"
            empty-model-value=""
            ghost
            @update:model-value="(v: string | number | null | undefined) => { rule.layerId = String(v ?? '') }"
          />
        </UFormField>
      </div>

      <div class="h-px bg-(--ui-border) w-full"></div>

      <div class="grid grid-cols-3 gap-4">
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('rules.tapLabel')"
              :hint="$t('rules.tapHint')"
              hint-visible-on="group-hover-rule"
            />
          </template>
          <RuleActionField :model-value="rule.tapAction" ghost :placeholder="$t('rules.tapPh')" @update:model-value="(v: string | null) => { rule.tapAction = v }" />
        </UFormField>

        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('rules.doubleTapLabel')"
              :hint="$t('rules.doubleTapHint')"
              hint-visible-on="group-hover-rule"
            />
          </template>
          <ActionPickerModal
            v-model="rule.doubleTapAction"
            allow-empty
            ghost
            :placeholder="$t('rules.doubleTapPh')"
            :clear-label="$t('common.clear')"
          />
        </UFormField>

        <div class="flex flex-col gap-3">
          <UFormField>
            <template #label>
              <FieldLabel
                :label="$t('rules.holdActionLabel')"
                :hint="$t('rules.holdActionHint')"
                hint-visible-on="group-hover-rule"
              />
            </template>
            <RuleActionField
              :model-value="rule.holdAction"
              key-only
              mode-kind="hold"
              ghost
              :placeholder="$t('rules.holdActionPh')"
              @update:model-value="updateHoldAction"
            />
          </UFormField>
          <UFormField v-if="rule.holdAction">
            <template #label>
              <FieldLabel
                :label="$t('rules.isolateLabel')"
                :hint="$t('rules.isolateHint')"
                hint-visible-on="group-hover-rule"
              />
            </template>
            <UInput
              v-model="rule.isolate"
              size="sm"
              :placeholder="$t('rules.isolatePh')"
            />
          </UFormField>
        </div>
      </div>
    </div>

    <div class="w-px bg-(--ui-border) self-stretch"></div>

    <div class="min-w-[12rem] max-w-[16rem] flex flex-col gap-4">
      <div class="flex items-center justify-between">
        <div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
          <AppTooltip :text="$t('common.moveUp')">
            <UButton
              icon="i-lucide-arrow-up"
              variant="ghost"
              color="neutral"
              size="sm"
              square
              :disabled="isFirst"
              @click="$emit('moveUp', rule.id)"
            />
          </AppTooltip>
          <AppTooltip :text="$t('common.moveDown')">
            <UButton
              icon="i-lucide-arrow-down"
              variant="ghost"
              color="neutral"
              size="sm"
              square
              :disabled="isLast"
              @click="$emit('moveDown', rule.id)"
            />
          </AppTooltip>
        </div>

        <div class="flex gap-1 items-center">
          <AppTooltip :text="rule.enabled === false ? $t('rules.enableRule') : $t('rules.disableRule')">
            <USwitch
              :model-value="rule.enabled !== false"
              size="sm"
              class="opacity-0 group-hover:opacity-100 transition-opacity duration-150"
              @update:model-value="(val: boolean) => { rule.enabled = val }"
            />
          </AppTooltip>

          <AppTooltip :text="$t('rules.deleteRule')">
            <UButton
              icon="i-lucide-trash-2"
              color="neutral"
              variant="ghost"
              size="sm"
              square
              class="opacity-0 group-hover:opacity-100 transition-opacity duration-150"
              :aria-label="$t('rules.deleteRule')"
              @click="$emit('remove', rule.id)"
            />
          </AppTooltip>
        </div>
      </div>

      <div class="flex flex-col gap-1 mt-1">
        <SettingTimeoutField
          v-model="rule.holdTimeoutMs"
          :label="$t('rules.holdLabel')"
          :hint="$t('rules.holdHint')"
          :default-value="defaultHoldTimeoutMs"
          :suffix="$t('common.ms')"
          hint-visible-on="group-hover-rule"
        />

        <SettingTimeoutField
          v-model="rule.doubleTapTimeoutMs"
          :label="$t('rules.doubleTapWindowLabel')"
          :hint="$t('rules.doubleTapWindowHint')"
          :default-value="defaultDoubleTapTimeoutMs"
          :suffix="$t('common.ms')"
          hint-visible-on="group-hover-rule"
        />
      </div>
    </div>
    <RuleConditionsModal v-model:open="isConditionsOpen" v-model:rule="rule" />
  </div>
</template>
