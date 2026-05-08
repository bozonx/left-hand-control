<script setup lang="ts">
import { ref, watch } from 'vue'
import type { Ref } from 'vue'
import type { LayerRule } from '~/types/config'
import ConditionsForm, { type ConditionsValue } from '~/components/features/shared/ConditionsForm.vue'
import AppTooltip from '~/components/shared/AppTooltip.vue'

const props = defineProps<{
  rule: LayerRule
}>()

const emit = defineEmits<{
  'update:rule': [rule: LayerRule]
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

function addToList(list: Ref<string[]>) {
  list.value = [...list.value, '']
}

function removeFromList(list: Ref<string[]>, index: number) {
  const next = [...list.value]
  next.splice(index, 1)
  list.value = next
}

function moveInList(list: Ref<string[]>, index: number, direction: -1 | 1) {
  const nextIndex = index + direction
  if (nextIndex < 0 || nextIndex >= list.value.length) return
  const next = [...list.value]
  const [item] = next.splice(index, 1)
  next.splice(nextIndex, 0, item)
  list.value = next
}

function updateInList(list: Ref<string[]>, index: number, value: string) {
  const next = [...list.value]
  next[index] = value
  list.value = next
}

function apply() {
  emit('update:rule', {
    ...props.rule,
    conditionGameMode: draftConditions.value.gameMode === 'ignore' ? undefined : draftConditions.value.gameMode,
    conditionLayouts: draftConditions.value.layouts.length > 0 ? draftConditions.value.layouts : undefined,
    conditionAppsWhitelist:
      draftAppsWhitelist.value.filter((s) => s.trim()).length > 0
        ? draftAppsWhitelist.value.filter((s) => s.trim())
        : undefined,
    conditionAppsBlacklist:
      draftAppsBlacklist.value.filter((s) => s.trim()).length > 0
        ? draftAppsBlacklist.value.filter((s) => s.trim())
        : undefined,
  })
  isOpen.value = false
}
</script>

<template>
  <UModal v-model:open="isOpen" :title="$t('rules.conditionsLabel')">
    <template #body>
      <div class="flex flex-col gap-6">
        <ConditionsForm v-model="draftConditions" :show-apps="false" />

        <div class="space-y-3 rounded-lg border border-(--ui-border) p-4">
          <div class="flex items-center justify-between gap-3">
            <div class="flex items-center gap-2">
              <h4 class="text-sm font-medium">{{ $t('rules.appsWhitelistLabel') }}</h4>
              <AppTooltip :text="$t('rules.appsWhitelistHint')" align="start" toggle-on-click>
                <UIcon name="i-lucide-info" class="h-4 w-4 cursor-help text-(--ui-text-muted)" />
              </AppTooltip>
            </div>
            <UButton type="button" icon="i-lucide-plus" size="sm" @click="addToList(draftAppsWhitelist)">
              {{ $t('common.add') }}
            </UButton>
          </div>

          <div v-if="draftAppsWhitelist.length > 0" class="space-y-2">
            <div
              v-for="(item, index) in draftAppsWhitelist"
              :key="index"
              class="flex items-center gap-2 rounded-lg border border-(--ui-border) p-3"
            >
              <UInput
                :model-value="item"
                class="flex-1"
                :placeholder="$t('rules.appsPlaceholder')"
                @update:model-value="updateInList(draftAppsWhitelist, index, String($event))"
              />
              <div class="flex items-center gap-1">
                <AppTooltip :text="$t('common.moveUp')">
                  <UButton
                    type="button"
                    color="neutral"
                    variant="ghost"
                    icon="i-lucide-arrow-up"
                    :disabled="index === 0"
                    :aria-label="$t('common.moveUp')"
                    @click="moveInList(draftAppsWhitelist, index, -1)"
                  />
                </AppTooltip>
                <AppTooltip :text="$t('common.moveDown')">
                  <UButton
                    type="button"
                    color="neutral"
                    variant="ghost"
                    icon="i-lucide-arrow-down"
                    :disabled="index === draftAppsWhitelist.length - 1"
                    :aria-label="$t('common.moveDown')"
                    @click="moveInList(draftAppsWhitelist, index, 1)"
                  />
                </AppTooltip>
                <UButton
                  type="button"
                  color="neutral"
                  variant="ghost"
                  icon="i-lucide-trash-2"
                  :aria-label="$t('common.delete')"
                  @click="removeFromList(draftAppsWhitelist, index)"
                />
              </div>
            </div>
          </div>
        </div>

        <div class="space-y-3 rounded-lg border border-(--ui-border) p-4">
          <div class="flex items-center justify-between gap-3">
            <div class="flex items-center gap-2">
              <h4 class="text-sm font-medium">{{ $t('rules.appsBlacklistLabel') }}</h4>
              <AppTooltip :text="$t('rules.appsBlacklistHint')" align="start" toggle-on-click>
                <UIcon name="i-lucide-info" class="h-4 w-4 cursor-help text-(--ui-text-muted)" />
              </AppTooltip>
            </div>
            <UButton type="button" icon="i-lucide-plus" size="sm" @click="addToList(draftAppsBlacklist)">
              {{ $t('common.add') }}
            </UButton>
          </div>

          <div v-if="draftAppsBlacklist.length > 0" class="space-y-2">
            <div
              v-for="(item, index) in draftAppsBlacklist"
              :key="index"
              class="flex items-center gap-2 rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/50 p-3"
            >
              <UInput
                :model-value="item"
                class="flex-1"
                :placeholder="$t('rules.appsPlaceholder')"
                @update:model-value="updateInList(draftAppsBlacklist, index, String($event))"
              />
              <div class="flex items-center gap-1">
                <AppTooltip :text="$t('common.moveUp')">
                  <UButton
                    type="button"
                    color="neutral"
                    variant="ghost"
                    icon="i-lucide-arrow-up"
                    :disabled="index === 0"
                    :aria-label="$t('common.moveUp')"
                    @click="moveInList(draftAppsBlacklist, index, -1)"
                  />
                </AppTooltip>
                <AppTooltip :text="$t('common.moveDown')">
                  <UButton
                    type="button"
                    color="neutral"
                    variant="ghost"
                    icon="i-lucide-arrow-down"
                    :disabled="index === draftAppsBlacklist.length - 1"
                    :aria-label="$t('common.moveDown')"
                    @click="moveInList(draftAppsBlacklist, index, 1)"
                  />
                </AppTooltip>
                <UButton
                  type="button"
                  color="neutral"
                  variant="ghost"
                  icon="i-lucide-trash-2"
                  :aria-label="$t('common.delete')"
                  @click="removeFromList(draftAppsBlacklist, index)"
                />
              </div>
            </div>
          </div>
        </div>
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
