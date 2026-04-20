<script setup lang="ts">
import RuleRow from '~/components/features/rules/RuleRow.vue'
import LayerEditorModal from '~/components/shared/LayerEditorModal.vue'
import { randomId } from '~/utils/keys'

const { config } = useConfig()
const { createLayer } = useLayers()

const layerOptions = computed(() =>
  config.value.layers
    .filter((l) => l.id !== 'base')
    .map((l) => ({ label: l.name, value: l.id })),
)

function addRule() {
  config.value.rules.push({
    id: randomId(),
    key: '',
    layerId: '',
    tapAction: '',
    holdAction: '',
    doubleTapAction: '',
    holdTimeoutMs: undefined,
    doubleTapTimeoutMs: undefined,
  })
}

function removeRule(id: string) {
  config.value.rules = config.value.rules.filter((r) => r.id !== id)
}

// --- New-layer modal -----------------------------------------------------
const newLayerOpen = ref(false)
const newLayerName = ref('')
const newLayerDescription = ref('')
const newLayerForRuleId = ref<string | null>(null)

function openNewLayer(ruleId: string) {
  newLayerForRuleId.value = ruleId
  newLayerName.value = ''
  newLayerDescription.value = ''
  newLayerOpen.value = true
}

function confirmNewLayer() {
  const id = createLayer({
    name: newLayerName.value,
    description: newLayerDescription.value,
  })
  if (!id) return
  if (newLayerForRuleId.value) {
    const rule = config.value.rules.find(
      (r) => r.id === newLayerForRuleId.value,
    )
    if (rule) rule.layerId = id
  }
  newLayerOpen.value = false
}
</script>

<template>
  <div class="space-y-6">
    <UCard>
      <template #header>
        <div class="flex items-center justify-between">
          <div>
            <h2 class="font-semibold">{{ $t('rules.title') }}</h2>
            <p class="text-xs text-(--ui-text-muted) mt-1">
              {{ $t('rules.subtitle') }}
            </p>
          </div>
          <UButton icon="i-lucide-plus" size="sm" @click="addRule">
            {{ $t('rules.addBtn') }}
          </UButton>
        </div>
      </template>

      <div v-if="config.rules.length === 0" class="text-sm text-(--ui-text-muted)">
        {{ $t('rules.empty') }}
      </div>

      <div v-else class="space-y-3">
        <RuleRow
          v-for="rule in config.rules"
          :key="rule.id"
          :rule="rule"
          :layer-options="layerOptions"
          :default-hold-timeout-ms="config.settings.defaultHoldTimeoutMs"
          :default-double-tap-timeout-ms="config.settings.defaultDoubleTapTimeoutMs"
          @remove="removeRule"
          @create-layer="openNewLayer"
        />
      </div>
    </UCard>

    <LayerEditorModal
      v-model="newLayerOpen"
      v-model:name="newLayerName"
      v-model:description="newLayerDescription"
      :title="$t('rules.newLayerTitle')"
      :confirm-label="$t('common.create')"
      :name-placeholder="$t('rules.layerNamePh')"
      :description-placeholder="$t('rules.layerDescPh')"
      @confirm="confirmNewLayer"
    />
  </div>
</template>
