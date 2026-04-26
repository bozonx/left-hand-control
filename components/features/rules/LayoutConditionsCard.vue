<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import LayoutConditionsModal from '~/components/features/settings/LayoutConditionsModal.vue'
import {
  isUserLayoutId,
  userLayoutNameFromId,
} from '~/composables/useLayoutLibrary'
import type { ConditionKind } from '~/composables/useLayoutConditions'
import type { LayoutConditionSet } from '~/types/config'

const { config, currentLayoutId } = useConfig()
const { t } = useI18n()

const modalOpen = ref(false)
const modalKind = ref<ConditionKind>('whitelist')

const layoutId = computed(() => currentLayoutId.value)
const layoutMode = computed(() => config.value.settings.layoutMode)

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
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <h2 class="text-sm font-semibold">
            {{ $t('rules.layoutConditionsTitle') }}
          </h2>
          <p class="text-xs text-(--ui-text-muted) mt-0.5">
            {{ layoutId ? $t('rules.layoutConditionsSubtitle', { name: layoutLabel }) : $t('rules.saveToConfigureAuto') }}
          </p>
        </div>
      </div>
    </template>

    <div v-if="layoutMode === 'auto'" class="space-y-3">
      <template v-if="layoutId">
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
      </template>
      <div v-else class="text-sm text-(--ui-text-muted)">
        {{ $t('rules.saveToConfigureAuto') }}
      </div>
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
