<script setup lang="ts">
import { computed, ref, watch } from 'vue'
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

const draft = ref<ConditionsValue>({
  gameMode: 'ignore',
  layouts: [],
  apps: [],
})

watch(isOpen, (open) => {
  if (!open) return
  const set = getConditionSet(props.layoutId, props.kind)
  draft.value = {
    gameMode: (set.gameMode ?? 'ignore') as 'on' | 'off' | 'ignore',
    layouts: [...set.layouts],
    apps: [...(set.apps ?? [])],
  }
})

function apply() {
  setConditionSet(props.layoutId, props.kind, {
    gameMode: draft.value.gameMode === 'ignore' ? undefined : draft.value.gameMode,
    layouts: draft.value.layouts,
    apps: draft.value.apps && draft.value.apps.length > 0 ? draft.value.apps : undefined,
  })
  isOpen.value = false
}

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
      <ConditionsForm v-model="draft" />
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
