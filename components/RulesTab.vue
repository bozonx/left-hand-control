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

const ruleIds = computed(() => config.value.rules.map((r) => r.id))
const { selectedId, select, containerRef } = useListKeyboardNavigation({
  ids: ruleIds,
  move: (id: string, delta: number) => moveRule(id, delta < 0 ? 'up' : 'down'),
})

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

      <div v-else ref="containerRef" class="space-y-2">
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
          :key-error="rule.id === newestRuleId && !rule.key ? $t('rules.keyRequired') : undefined"
          :selected="selectedId === rule.id"
          @select="select(rule.id)"
          @remove="requestRemoveRule"
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
          <UButton color="error" icon="i-lucide-trash-2" @click="confirmRemoveRule">
            {{ $t('common.delete') }}
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
