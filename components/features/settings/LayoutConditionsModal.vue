<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import ConditionsForm, {
  type ConditionsValue,
} from '~/components/features/shared/ConditionsForm.vue'
import {
  useLayoutConditions,
  type ConditionKind,
} from '~/composables/useLayoutConditions'

const props = defineProps<{
  layoutId: string
  kind: ConditionKind
  layoutLabel?: string
}>()

const isOpen = defineModel<boolean>('open', { default: false })

const { t } = useI18n()
const { getConditionSet, setConditionSet } = useLayoutConditions()

const conditions = computed<ConditionsValue>({
  get: (): ConditionsValue => {
    const set = getConditionSet(props.layoutId, props.kind)
    return {
      gameMode: (set.gameMode ?? 'ignore') as 'on' | 'off' | 'ignore',
      layouts: [...set.layouts],
    }
  },
  set: (value: ConditionsValue) => {
    setConditionSet(props.layoutId, props.kind, {
      gameMode: value.gameMode === 'ignore' ? undefined : value.gameMode,
      layouts: value.layouts,
    })
  },
})

const title = computed(() => {
  const base =
    props.kind === 'whitelist'
      ? t('settings.whitelistTitle')
      : t('settings.blacklistTitle')
  return props.layoutLabel ? `${base} — ${props.layoutLabel}` : base
})
</script>

<template>
  <UModal v-model:open="isOpen" :title="title">
    <template #body>
      <p class="text-sm text-(--ui-text-muted) mb-4">
        {{
          kind === 'whitelist'
            ? t('settings.whitelistHint')
            : t('settings.blacklistHint')
        }}
      </p>
      <ConditionsForm v-model="conditions" />
    </template>

    <template #footer>
      <div class="flex justify-end w-full">
        <UButton @click="isOpen = false">{{ $t('common.close') }}</UButton>
      </div>
    </template>
  </UModal>
</template>
