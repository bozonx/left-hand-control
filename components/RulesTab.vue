<script setup lang="ts">
import RuleRow from '~/components/features/rules/RuleRow.vue'
import LayerEditorModal from '~/components/shared/LayerEditorModal.vue'
const {
  config,
  layerOptions,
  newestRuleId,
  addRule,
  removeRule,
  moveRule,
  newLayerOpen,
  newLayerName,
  newLayerDescription,
  openNewLayer,
  confirmNewLayer,
  markRuleConfigured,
} = useRulesEditor()

const props = defineProps<{
  showBackToTop?: boolean
}>()

const emit = defineEmits<{
  backToTop: []
}>()

function addRuleFromFooter() {
  addRule()
  emit('backToTop')
}
</script>

<template>
  <div class="space-y-4">
    <UCard>
      <template #header>
        <div class="flex items-center justify-between gap-3">
          <div>
            <h2 class="text-sm font-semibold">{{ $t('rules.title') }}</h2>
            <p class="text-xs text-(--ui-text-muted) mt-0.5">
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

      <div v-else class="space-y-2">
        <RuleRow
          v-for="(rule, index) in config.rules"
          :key="rule.id"
          :rule="rule"
          :layer-options="layerOptions"
          :default-hold-timeout-ms="config.settings.defaultHoldTimeoutMs"
          :default-double-tap-timeout-ms="config.settings.defaultDoubleTapTimeoutMs"
          :is-first="index === 0"
          :is-last="index === config.rules.length - 1"
          :is-new="rule.id === newestRuleId"
          @remove="removeRule"
          @move-up="moveRule($event, 'up')"
          @move-down="moveRule($event, 'down')"
          @create-layer="openNewLayer"
          @key-selected="markRuleConfigured"
        />
      </div>

      <template #footer>
        <div class="flex items-center justify-between gap-3">
          <UButton icon="i-lucide-plus" size="sm" @click="addRuleFromFooter">
            {{ $t('rules.addBtn') }}
          </UButton>
          <ULink
            v-if="showBackToTop"
            class="text-xs text-(--ui-text-muted) hover:text-(--ui-primary) transition-colors cursor-pointer"
            @click="emit('backToTop')"
          >
            {{ $t('common.backToTop') }}
          </ULink>
        </div>
      </template>
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
