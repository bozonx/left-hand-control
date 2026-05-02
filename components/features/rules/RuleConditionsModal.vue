<script setup lang="ts">
import { ref, watch } from 'vue'
import type { LayerRule } from '~/types/config'
import ConditionsForm, { type ConditionsValue } from '~/components/features/shared/ConditionsForm.vue'
import AppsListField from '~/components/features/shared/AppsListField.vue'

const props = defineProps<{
  rule: LayerRule
}>()

const isOpen = defineModel<boolean>('open', { default: false })

const draftConditions = ref<ConditionsValue>({
  gameMode: 'ignore',
  layouts: [],
})

const draftAppsWhitelist = ref<string[]>([])
const draftAppsBlacklist = ref<string[]>([])

watch(isOpen, (open) => {
  if (!open) return
  draftConditions.value = {
    gameMode: props.rule.conditionGameMode ?? 'ignore',
    layouts: [...(props.rule.conditionLayouts ?? [])],
  }
  draftAppsWhitelist.value = [...(props.rule.conditionAppsWhitelist ?? [])]
  draftAppsBlacklist.value = [...(props.rule.conditionAppsBlacklist ?? [])]
})

function apply() {
  props.rule.conditionGameMode = draftConditions.value.gameMode === 'ignore' ? undefined : draftConditions.value.gameMode
  props.rule.conditionLayouts = draftConditions.value.layouts.length > 0 ? draftConditions.value.layouts : undefined
  props.rule.conditionAppsWhitelist = draftAppsWhitelist.value.length > 0 ? draftAppsWhitelist.value : undefined
  props.rule.conditionAppsBlacklist = draftAppsBlacklist.value.length > 0 ? draftAppsBlacklist.value : undefined
  isOpen.value = false
}
</script>

<template>
  <UModal v-model:open="isOpen" :title="$t('rules.conditionsLabel')">
    <template #body>
      <div class="flex flex-col gap-6">
        <ConditionsForm v-model="draftConditions" :show-apps="false" />

        <AppsListField
          v-model="draftAppsWhitelist"
          :label="$t('rules.appsWhitelistLabel')"
          :hint="$t('rules.appsWhitelistHint')"
          :placeholder="$t('rules.appsPlaceholder')"
        />

        <AppsListField
          v-model="draftAppsBlacklist"
          :label="$t('rules.appsBlacklistLabel')"
          :hint="$t('rules.appsBlacklistHint')"
          :placeholder="$t('rules.appsPlaceholder')"
        />
      </div>
    </template>

    <template #footer>
      <div class="flex justify-end gap-2 w-full">
        <UButton color="neutral" variant="ghost" @click="isOpen = false">
          {{ $t('common.cancel') }}
        </UButton>
        <UButton @click="apply">{{ $t('common.apply') }}</UButton>
      </div>
    </template>
  </UModal>
</template>
