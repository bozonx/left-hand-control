<script setup lang="ts">
import { computed } from 'vue'
import type { LayerRule } from '~/types/config'
import ConditionsForm, { type ConditionsValue } from '~/components/features/shared/ConditionsForm.vue'
import AppsListField from '~/components/features/shared/AppsListField.vue'

const props = defineProps<{
  rule: LayerRule
}>()

const isOpen = defineModel<boolean>('open', { default: false })

// Initialize defaults if undefined
if (!props.rule.conditionGameMode) {
  props.rule.conditionGameMode = 'ignore'
}
if (!props.rule.conditionLayouts) {
  props.rule.conditionLayouts = []
}

const conditions = computed<ConditionsValue>({
  get: () => ({
    gameMode: props.rule.conditionGameMode ?? 'ignore',
    layouts: props.rule.conditionLayouts ?? [],
  }),
  set: (value) => {
    props.rule.conditionGameMode = value.gameMode
    props.rule.conditionLayouts = value.layouts
  },
})

const appsWhitelist = computed<string[]>({
  get: () => props.rule.conditionAppsWhitelist ?? [],
  set: (value) => {
    props.rule.conditionAppsWhitelist = value.length > 0 ? value : undefined
  },
})

const appsBlacklist = computed<string[]>({
  get: () => props.rule.conditionAppsBlacklist ?? [],
  set: (value) => {
    props.rule.conditionAppsBlacklist = value.length > 0 ? value : undefined
  },
})
</script>

<template>
  <UModal v-model:open="isOpen" :title="$t('rules.conditionsLabel')">
    <template #body>
      <div class="flex flex-col gap-6">
        <ConditionsForm v-model="conditions" :show-apps="false" />

        <AppsListField
          v-model="appsWhitelist"
          :label="$t('rules.appsWhitelistLabel')"
          :hint="$t('rules.appsWhitelistHint')"
          :placeholder="$t('rules.appsPlaceholder')"
        />

        <AppsListField
          v-model="appsBlacklist"
          :label="$t('rules.appsBlacklistLabel')"
          :hint="$t('rules.appsBlacklistHint')"
          :placeholder="$t('rules.appsPlaceholder')"
        />
      </div>
    </template>

    <template #footer>
      <div class="flex justify-end w-full">
        <UButton @click="isOpen = false">{{ $t('common.close') }}</UButton>
      </div>
    </template>
  </UModal>
</template>
