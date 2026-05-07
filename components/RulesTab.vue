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
  openNewLayer,
  confirmNewLayer,
  markRuleConfigured,
} = useRulesEditor()

const deleteConfirmOpen = ref(false)
const pendingDeleteRuleId = ref<string | null>(null)

function requestRemoveRule(id: string) {
  const rule = config.value.rules.find((item) => item.id === id)
  if (!rule?.key) {
    removeRule(id)
    return
  }
  pendingDeleteRuleId.value = id
  deleteConfirmOpen.value = true
}

function confirmRemoveRule() {
  if (pendingDeleteRuleId.value) removeRule(pendingDeleteRuleId.value)
  pendingDeleteRuleId.value = null
  deleteConfirmOpen.value = false
}

function cancelRemoveRule() {
  pendingDeleteRuleId.value = null
  deleteConfirmOpen.value = false
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

      <div v-if="config.rules.length === 0" class="py-8 text-center space-y-2">
        <UIcon name="i-lucide-clipboard-list" class="w-8 h-8 text-(--ui-text-muted) mx-auto" />
        <p class="text-sm text-(--ui-text-muted)">
          {{ $t('rules.empty') }}
        </p>
      </div>

      <div v-else class="space-y-2">
        <RuleRow
          v-for="(rule, index) in config.rules"
          :key="rule.id"
          v-model:rule="config.rules[index]"
          :layer-options="layerOptions"
          :default-hold-timeout-ms="config.settings.defaultHoldTimeoutMs"
          :default-double-tap-timeout-ms="config.settings.defaultDoubleTapTimeoutMs"
          :is-first="index === 0"
          :is-last="index === config.rules.length - 1"
          :is-new="rule.id === newestRuleId"
          :key-error="!rule.key ? $t('rules.keyRequired') : undefined"
          @remove="requestRemoveRule"
          @move-up="moveRule($event, 'up')"
          @move-down="moveRule($event, 'down')"
          @create-layer="openNewLayer"
          @key-selected="markRuleConfigured"
        />
      </div>

    </UCard>

    <LayerEditorModal
      v-model="newLayerOpen"
      v-model:name="newLayerName"
      :title="$t('rules.newLayerTitle')"
      :confirm-label="$t('common.create')"
      :name-placeholder="$t('rules.layerNamePh')"
      @confirm="confirmNewLayer"
    />

    <UModal v-model:open="deleteConfirmOpen" :title="$t('rules.confirmDeleteTitle')">
      <template #body>
        <p class="text-sm">
          {{ $t('rules.confirmDeleteBody') }}
        </p>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton variant="ghost" color="neutral" @click="cancelRemoveRule">
            {{ $t('common.cancel') }}
          </UButton>
          <UButton color="error" icon="i-lucide-trash-2" autofocus @click="confirmRemoveRule">
            {{ $t('common.delete') }}
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
