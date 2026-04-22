<script setup lang="ts">
import ResettableSelectMenu from '~/components/shared/ResettableSelectMenu.vue'

type ModeKind = 'native' | 'none' | 'action'

const props = defineProps<{
  placeholder: string
  keyOnly?: boolean
}>()

const model = defineModel<string | null>({ default: '' })
const { t } = useI18n()

const mode = ref<ModeKind>('native')

const modeItems = computed(() => [
  { label: t('rules.modeNative'), value: 'native' },
  { label: t('rules.modeNone'), value: 'none' },
  { label: t('rules.modeAction'), value: 'action' },
])

watch(
  model,
  (value) => {
    if (value === null) {
      mode.value = 'none'
      return
    }
    if (value) {
      mode.value = 'action'
      return
    }
    if (mode.value !== 'action') mode.value = 'native'
  },
  { immediate: true },
)

watch(mode, (value) => {
  if (value === 'native') {
    model.value = ''
    return
  }
  if (value === 'none') {
    model.value = null
    return
  }
  if (model.value === null) model.value = ''
})

function updateAction(value: string) {
  model.value = value
}
</script>

<template>
  <div class="space-y-1.5">
    <ResettableSelectMenu
      v-model="mode"
      :items="modeItems"
      value-key="value"
      :reset-value="'native'"
    />
    <ActionPickerModal
      v-if="mode === 'action'"
      :model-value="typeof model === 'string' ? model : ''"
      :key-only="props.keyOnly"
      allow-empty
      :placeholder="placeholder"
      @update:model-value="updateAction"
    />
  </div>
</template>
