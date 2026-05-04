<script setup lang="ts">
import FieldLabel from '~/components/FieldLabel.vue'
import type { GameModeProcessMatcher } from '~/types/config'

const { t } = useI18n()
const props = defineProps<{
  useGamemoded: boolean
  useFullscreen: boolean
  processMatchers: GameModeProcessMatcher[]
}>()

const emit = defineEmits<{
  'update:useGamemoded': [value: boolean]
  'update:useFullscreen': [value: boolean]
  'update:processMatchers': [value: GameModeProcessMatcher[]]
}>()

const advancedOpen = ref(false)
const newProcessName = ref('')

const matchModeItems = computed(() => [
  { label: t('settings.gameModeMatchSubstring'), value: 'substring' as const },
  { label: t('settings.gameModeMatchExact'), value: 'exact' as const },
])

const detectionEnabled = computed(
  () =>
    props.useGamemoded ||
    props.useFullscreen ||
    props.processMatchers.some((item) => item.name.trim().length > 0),
)

function updateMatcher(id: string, patch: Partial<GameModeProcessMatcher>) {
  emit(
    'update:processMatchers',
    props.processMatchers.map((item) => (item.id === id ? { ...item, ...patch } : item)),
  )
}

function removeMatcher(id: string) {
  emit(
    'update:processMatchers',
    props.processMatchers.filter((item) => item.id !== id),
  )
}

function addMatcher() {
  const name = newProcessName.value.trim()
  if (!name) return
  emit('update:processMatchers', [
    ...props.processMatchers,
    {
      id: `process-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      name,
      matchMode: 'substring',
      onlyActiveWindow: true,
    },
  ])
  newProcessName.value = ''
}
</script>

<template>
  <UCard variant="outline">
    <template #header>
      <div>
        <div class="flex items-center gap-2">
          <UIcon name="i-lucide-gamepad-2" class="h-5 w-5 text-(--ui-primary)" />
          <h2 class="text-sm font-semibold">{{ $t('settings.gameModeTitle') }}</h2>
        </div>
        <p class="mt-1 text-sm text-(--ui-text-muted)">
          {{ $t('settings.gameModeSubtitle') }}
        </p>
      </div>
    </template>

    <div class="space-y-4">
      <div class="grid gap-4 sm:grid-cols-2">
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.gameModeUseGamemoded')"
              :hint="$t('settings.gameModeUseGamemodedHint')"
            />
          </template>
          <UCheckbox
            :model-value="useGamemoded"
            @update:model-value="$emit('update:useGamemoded', $event as boolean)"
          />
        </UFormField>

        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.gameModeUseFullscreen')"
              :hint="$t('settings.gameModeUseFullscreenHint')"
            />
          </template>
          <UCheckbox
            :model-value="useFullscreen"
            @update:model-value="$emit('update:useFullscreen', $event as boolean)"
          />
        </UFormField>
      </div>

      <div class="rounded-lg bg-(--ui-bg-muted) p-3 text-xs text-(--ui-text-muted)">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-info" class="mt-0.5 h-4 w-4 shrink-0" />
          <p>
            {{ detectionEnabled ? $t('settings.gameModeInfo') : $t('settings.gameModeDisabledInfo') }}
          </p>
        </div>
      </div>

      <UButton
        type="button"
        variant="link"
        color="primary"
        :icon="advancedOpen ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'"
        class="px-0"
        @click="advancedOpen = !advancedOpen"
      >
        {{ $t('settings.gameModeAdvanced') }}
      </UButton>

      <div v-if="advancedOpen" class="space-y-3 border-t border-(--ui-border) pt-4">
        <div class="flex gap-2">
          <UInput
            v-model="newProcessName"
            class="min-w-0 flex-1"
            :placeholder="$t('settings.gameModeProcessPlaceholder')"
            @keydown.enter.prevent="addMatcher"
          />
          <UButton
            type="button"
            icon="i-lucide-plus"
            :disabled="!newProcessName.trim()"
            @click="addMatcher"
          >
            {{ $t('settings.gameModeAddProcess') }}
          </UButton>
        </div>

        <div v-if="processMatchers.length > 0" class="space-y-2">
          <div
            v-for="item in processMatchers"
            :key="item.id"
            class="grid gap-2 rounded-lg border border-(--ui-border) p-3 lg:grid-cols-[minmax(0,1fr)_12rem_12rem_auto] lg:items-center"
          >
            <UInput
              :model-value="item.name"
              @update:model-value="updateMatcher(item.id, { name: String($event) })"
            />
            <USelectMenu
              :model-value="item.matchMode"
              :items="matchModeItems"
              value-key="value"
              @update:model-value="updateMatcher(item.id, { matchMode: $event as GameModeProcessMatcher['matchMode'] })"
            />
            <UCheckbox
              :model-value="item.onlyActiveWindow"
              :label="$t('settings.gameModeOnlyActiveWindow')"
              @update:model-value="updateMatcher(item.id, { onlyActiveWindow: $event as boolean })"
            />
            <UButton
              type="button"
              color="neutral"
              variant="ghost"
              icon="i-lucide-trash-2"
              :aria-label="$t('settings.gameModeRemoveProcess')"
              @click="removeMatcher(item.id)"
            />
          </div>
        </div>
      </div>
    </div>
  </UCard>
</template>
