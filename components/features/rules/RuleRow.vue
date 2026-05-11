<script setup lang="ts">
import { computed, ref } from 'vue'
import type { LayerRule } from '~/types/config'
import type { RuleIssue } from '~/utils/ruleDiagnostics'
import { ruleIssueMessageKey } from '~/utils/ruleDiagnostics'
import SettingTimeoutField from '~/components/SettingTimeoutField.vue'
import ResettableSelectMenu from '~/components/shared/ResettableSelectMenu.vue'
import RuleActionField from '~/components/features/rules/RuleActionField.vue'
import AppTooltip from '~/components/shared/AppTooltip.vue'
import RuleConditionsModal from './RuleConditionsModal.vue'

defineOptions({
  inheritAttrs: false
})

const props = defineProps<{
  layerOptions: Array<{ label: string, value: string }>
  defaultHoldTimeoutMs: number
  defaultDoubleTapTimeoutMs: number
  isFirst?: boolean
  isLast?: boolean
  isNew?: boolean
  keyError?: string
  issues?: RuleIssue[]
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

const hasConditions = computed(() => Boolean(
  (rule.value.conditionGameMode && rule.value.conditionGameMode !== 'ignore')
  || (rule.value.conditionLayouts?.length ?? 0) > 0
  || (rule.value.conditionAppsWhitelist?.length ?? 0) > 0
  || (rule.value.conditionAppsBlacklist?.length ?? 0) > 0
))

const hasErrors = computed(() => (props.issues ?? []).some((issue) => issue.severity === 'error'))
const hasWarnings = computed(() => (props.issues ?? []).some((issue) => issue.severity === 'warning'))

function clearConditions() {
  rule.value.conditionGameMode = undefined
  rule.value.conditionLayouts = undefined
  rule.value.conditionAppsWhitelist = undefined
  rule.value.conditionAppsBlacklist = undefined
}

function appBadgeLabel(value: string) {
  return value.trim().slice(0, 8)
}

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
        : 'border-(--ui-border) bg-(--ui-bg-muted)/40 hover:bg-(--ui-bg-muted)/60 hover:border-(--ui-primary)/50 hover:shadow-(--ui-primary)/5',
      hasErrors
        ? 'border-(--ui-error)/50 bg-(--ui-error)/5 hover:border-(--ui-error)/60 hover:shadow-(--ui-error)/10'
        : hasWarnings
          ? 'border-(--ui-warning)/50 bg-(--ui-warning)/5 hover:border-(--ui-warning)/60 hover:shadow-(--ui-warning)/10'
          : ''
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
          <div class="flex gap-2">
            <UButton
              color="neutral"
              :variant="hasConditions ? 'soft' : 'ghost'"
              class="h-8 min-w-0 flex-1 justify-start font-normal"
              :class="hasConditions ? '' : 'border border-dashed border-(--ui-border) text-(--ui-text-muted) hover:text-(--ui-text) hover:border-(--ui-border-accent) hover:bg-(--ui-bg-elevated)/50'"
              @click="isConditionsOpen = true"
            >
              <span class="truncate">{{ hasConditions ? $t('rules.editConditions') : $t('common.notSet') }}</span>
            </UButton>
            <UButton
              v-if="hasConditions"
              color="neutral"
              variant="ghost"
              icon="i-lucide-x"
              size="sm"
              :aria-label="$t('rules.clearConditions')"
              @click="clearConditions"
            >
              {{ $t('common.clear') }}
            </UButton>
          </div>
          <div
            v-if="hasConditions"
            class="flex flex-wrap gap-1 mt-1.5"
          >
            <UBadge
              v-if="rule.conditionGameMode && rule.conditionGameMode !== 'ignore'"
              size="xs"
              variant="subtle"
              color="primary"
            >
              <UIcon name="i-lucide-gamepad-2" class="mr-1 h-3 w-3" />
              {{ rule.conditionGameMode === 'on' ? $t('rules.gameModeOn') : $t('rules.gameModeOff') }}
            </UBadge>
            <UBadge
              v-for="layoutCode in rule.conditionLayouts"
              :key="`layout-${layoutCode}`"
              size="xs"
              variant="subtle"
              color="neutral"
            >
              {{ layoutCode }}
            </UBadge>
            <UBadge
              v-for="(app, index) in rule.conditionAppsWhitelist?.filter((value) => value.trim())"
              :key="`whitelist-${index}-${app}`"
              size="xs"
              variant="subtle"
              color="success"
            >
              {{ appBadgeLabel(app) }}
            </UBadge>
            <UBadge
              v-for="(app, index) in rule.conditionAppsBlacklist?.filter((value) => value.trim())"
              :key="`blacklist-${index}-${app}`"
              size="xs"
              variant="subtle"
              color="error"
            >
              {{ appBadgeLabel(app) }}
            </UBadge>
          </div>
        </UFormField>

        <div class="min-w-0 space-y-1.5">
          <div class="flex min-h-5 w-full items-center justify-between gap-2">
            <FieldLabel
              :label="$t('rules.layerLabel')"
              :hint="$t('rules.layerHint')"
              hint-visible-on="group-hover-rule"
            />
            <ULink
              class="shrink-0 text-right text-xs text-(--ui-text-muted) opacity-0 group-hover/rule:opacity-100 hover:text-(--ui-primary) transition-all duration-200 cursor-pointer"
              @click="$emit('createLayer', rule.id)"
            >
              {{ $t('rules.createLayer') }}
            </ULink>
          </div>
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
        </div>
      </div>

      <div
        v-if="issues?.length"
        class="flex flex-wrap gap-1.5"
      >
        <UBadge
          v-for="issue in issues"
          :key="`${issue.code}-${issue.ruleId}`"
          size="xs"
          :color="issue.severity === 'error' ? 'error' : 'warning'"
          variant="subtle"
        >
          <UIcon
            :name="issue.severity === 'error' ? 'i-lucide-circle-alert' : 'i-lucide-triangle-alert'"
            class="mr-1 h-3 w-3"
          />
          {{ $t(ruleIssueMessageKey(issue), { trigger: issue.trigger ?? rule.key }) }}
        </UBadge>
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
