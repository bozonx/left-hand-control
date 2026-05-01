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
  rule: LayerRule
  layerOptions: Array<{ label: string, value: string }>
  defaultHoldTimeoutMs: number
  defaultDoubleTapTimeoutMs: number
  isFirst?: boolean
  isLast?: boolean
  isNew?: boolean
  keyError?: string
  selected?: boolean
}>()

const emit = defineEmits<{
  select: []
  remove: [id: string]
  moveUp: [id: string]
  moveDown: [id: string]
  createLayer: [ruleId: string]
  keySelected: [id: string]
}>()

function onCardClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null
  if (target?.closest('input, textarea, select, button, [role="dialog"], [role="listbox"]')) return
  emit('select')
}

const isConditionsOpen = ref(false)
</script>

<template>
  <div
    class="relative p-4 rounded-xl border flex gap-6 group group/rule transition-all duration-150 hover:shadow-lg cursor-pointer"
    :class="[
      rule.enabled === false ? 'opacity-50 grayscale-[30%]' : '',
      selected
        ? 'border-(--ui-primary) ring-1 ring-(--ui-primary) bg-(--ui-bg-muted)/60 shadow-lg shadow-(--ui-primary)/5'
        : isNew
          ? 'border-sky-500/60 bg-sky-500/8 ring-1 ring-sky-500/20 hover:bg-sky-500/12 hover:border-sky-500/70 hover:shadow-sky-500/10'
          : 'border-(--ui-border) bg-(--ui-bg-muted)/40 hover:bg-(--ui-bg-muted)/60 hover:border-sky-500/50 hover:shadow-sky-500/5'
    ]"
    @click="onCardClick"
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
            :model-value="rule.key"
            key-only
            :invalid="!!keyError"
            :placeholder="$t('rules.keyPh')"
            @update:model-value="(value: string | null) => { rule.key = value ?? ''; if (value) $emit('keySelected', rule.id) }"
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
            <span class="inline-flex items-center gap-2">
              <FieldLabel
                :label="$t('rules.layerLabel')"
                :hint="$t('rules.layerHint')"
                hint-visible-on="group-hover-rule"
              />
              <ULink
                class="text-xs text-(--ui-text-muted) opacity-0 group-hover/rule:opacity-100 hover:text-(--ui-primary) transition-all duration-200 cursor-pointer"
                @click="$emit('createLayer', rule.id)"
              >
                {{ $t('rules.createLayer') }}
              </ULink>
            </span>
          </template>
          <ResettableSelectMenu
            v-model="rule.layerId"
            :items="layerOptions"
            value-key="value"
            :placeholder="$t('common.none')"
            reset-value=""
            :reset-aria-label="$t('rules.clearLayer')"
            empty-item-value="__none__"
            empty-model-value=""
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
          <RuleActionField v-model="rule.tapAction" :placeholder="$t('rules.tapPh')" />
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
            :placeholder="$t('rules.doubleTapPh')"
            :clear-label="$t('common.clear')"
          />
        </UFormField>

        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('rules.holdActionLabel')"
              :hint="$t('rules.holdActionHint')"
              hint-visible-on="group-hover-rule"
            />
          </template>
          <RuleActionField
            v-model="rule.holdAction"
            key-only
            mode-kind="hold"
            :placeholder="$t('rules.holdActionPh')"
          />
        </UFormField>
      </div>
    </div>

    <div class="w-px bg-(--ui-border) self-stretch"></div>

    <div class="min-w-[12rem] max-w-[16rem] flex flex-col gap-4">
      <div class="flex items-center justify-between">
        <div class="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
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

        <div class="flex gap-1 items-center">
          <AppTooltip :text="rule.enabled === false ? $t('rules.enableRule') : $t('rules.disableRule')">
            <USwitch
              :model-value="rule.enabled !== false"
              @update:model-value="(val: boolean) => rule.enabled = val"
              size="sm"
              class="opacity-0 group-hover:opacity-100 transition-opacity duration-150"
            />
          </AppTooltip>

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
    <RuleConditionsModal :rule="rule" v-model:open="isConditionsOpen" />
  </div>
</template>
