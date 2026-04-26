<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import LayoutConditionsModal from '~/components/features/settings/LayoutConditionsModal.vue'
import {
  isUserLayoutId,
  userLayoutNameFromId,
} from '~/composables/useLayoutLibrary'
import {
  useLayoutConditions,
  type ConditionKind,
} from '~/composables/useLayoutConditions'
import type { LayoutConditionSet } from '~/types/config'

const { config, currentLayoutId } = useConfig()
const { setIncludedInAuto, setAsDefault } = useLayoutConditions()
const { t } = useI18n()

const modalOpen = ref(false)
const modalKind = ref<ConditionKind>('whitelist')

const layoutId = computed(() => currentLayoutId.value)

const layoutLabel = computed(() => {
  const id = layoutId.value
  if (!id) return ''
  return isUserLayoutId(id) ? userLayoutNameFromId(id) : id
})

const rule = computed(() => {
  const id = layoutId.value
  if (!id) return undefined
  return config.value.settings.layoutConditions[id]
})

const isDefault = computed(
  () => !!layoutId.value && config.value.settings.autoDefaultLayoutId === layoutId.value,
)

const includedInAuto = computed(() => !!rule.value?.includedInAuto)

const hasConditions = computed(
  () => !!rule.value?.whitelist || !!rule.value?.blacklist,
)

function summarize(set: LayoutConditionSet | undefined): string {
  if (!set) return t('rules.conditionsNone')
  const parts: string[] = []
  if (set.gameMode === 'on') parts.push(t('rules.gameModeOn'))
  else if (set.gameMode === 'off') parts.push(t('rules.gameModeOff'))
  if (set.layouts.length > 0) parts.push(set.layouts.join(', '))
  return parts.length > 0 ? parts.join(' · ') : t('rules.conditionsNone')
}

const whitelistSummary = computed(() => summarize(rule.value?.whitelist))
const blacklistSummary = computed(() => summarize(rule.value?.blacklist))

function openWhitelist() {
  if (!layoutId.value || isDefault.value) return
  modalKind.value = 'whitelist'
  modalOpen.value = true
}

function openBlacklist() {
  if (!layoutId.value || isDefault.value) return
  modalKind.value = 'blacklist'
  modalOpen.value = true
}

function toggleAuto(value: boolean) {
  if (!layoutId.value) return
  setIncludedInAuto(layoutId.value, value)
}

function toggleDefault(value: boolean) {
  if (!layoutId.value) return
  setAsDefault(value ? layoutId.value : undefined)
}
</script>

<template>
  <UCard v-if="layoutId">
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <h2 class="text-sm font-semibold">
            {{ $t('rules.layoutConditionsTitle') }}
          </h2>
          <p class="text-xs text-(--ui-text-muted) mt-0.5">
            {{ $t('rules.layoutConditionsSubtitle', { name: layoutLabel }) }}
          </p>
        </div>
      </div>
    </template>

    <div class="space-y-3">
      <div class="flex items-center justify-between gap-3 flex-wrap">
        <div class="min-w-0">
          <div class="text-sm font-medium">
            {{ $t('rules.autoIncludeLabel') }}
          </div>
          <div class="text-xs text-(--ui-text-muted)">
            {{ $t('rules.autoIncludeHint') }}
          </div>
        </div>
        <UToggle
          :model-value="includedInAuto"
          :disabled="hasConditions || isDefault"
          @update:model-value="(v: boolean) => toggleAuto(v === true)"
        />
      </div>

      <div class="flex items-center justify-between gap-3 flex-wrap">
        <div class="min-w-0">
          <div class="text-sm font-medium flex items-center gap-2">
            {{ $t('rules.autoDefaultLabel') }}
            <UBadge
              v-if="isDefault"
              color="primary"
              variant="subtle"
              size="sm"
              icon="i-lucide-star"
            >
              {{ $t('settings.defaultBadge') }}
            </UBadge>
          </div>
          <div class="text-xs text-(--ui-text-muted)">
            {{ $t('rules.autoDefaultHint') }}
          </div>
        </div>
        <UToggle
          :model-value="isDefault"
          :disabled="hasConditions"
          @update:model-value="(v: boolean) => toggleDefault(v === true)"
        />
      </div>

      <div
        v-if="isDefault"
        class="text-xs p-2 rounded border border-(--ui-info)/40 bg-(--ui-info)/10 text-(--ui-text-muted)"
      >
        {{ $t('rules.autoDefaultLockHint') }}
      </div>

      <UButton
        block
        color="neutral"
        variant="outline"
        :disabled="isDefault"
        class="justify-between"
        @click="openWhitelist"
      >
        <div class="flex items-center gap-2 min-w-0">
          <UIcon name="i-lucide-list-checks" class="shrink-0" />
          <span class="font-medium">{{ $t('settings.whitelist') }}</span>
          <span class="text-(--ui-text-muted) truncate">
            {{ whitelistSummary }}
          </span>
        </div>
        <UIcon name="i-lucide-chevron-right" class="shrink-0" />
      </UButton>

      <UButton
        block
        color="neutral"
        variant="outline"
        :disabled="isDefault"
        class="justify-between"
        @click="openBlacklist"
      >
        <div class="flex items-center gap-2 min-w-0">
          <UIcon name="i-lucide-list-x" class="shrink-0" />
          <span class="font-medium">{{ $t('settings.blacklist') }}</span>
          <span class="text-(--ui-text-muted) truncate">
            {{ blacklistSummary }}
          </span>
        </div>
        <UIcon name="i-lucide-chevron-right" class="shrink-0" />
      </UButton>
    </div>

    <LayoutConditionsModal
      v-if="layoutId"
      v-model:open="modalOpen"
      :layout-id="layoutId"
      :kind="modalKind"
      :layout-label="layoutLabel"
    />
  </UCard>
</template>
