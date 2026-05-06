<script setup lang="ts">
import type { PlatformInfo } from '~/types/platform'
import type { LinuxWaylandTextMode } from '~/types/config'

const props = defineProps<{
  platform: PlatformInfo | null
}>()

const { config } = useConfig()
const { t } = useI18n()

const isLinuxWayland = computed(() => !!props.platform?.linux?.has_wayland)

const textModeItems = computed(() => [
  {
    value: 'keycode' as LinuxWaylandTextMode,
    label: t('settings.system.textModeKeycode'),
    description: t('settings.system.textModeKeycodeHint'),
  },
  {
    value: 'clipboard' as LinuxWaylandTextMode,
    label: t('settings.system.textModeClipboard'),
    description: t('settings.system.textModeClipboardHint'),
  },
])

const textMode = computed({
  get: () => config.value.settings.linuxWaylandTextMode ?? 'keycode',
  set: (value: LinuxWaylandTextMode) => {
    config.value.settings.linuxWaylandTextMode = value
  },
})
</script>

<template>
  <UCard v-if="isLinuxWayland">
    <template #header>
      <h2 class="text-sm font-semibold">{{ $t('settings.system.title') }}</h2>
    </template>

    <div class="space-y-4">
      <p class="text-xs text-(--ui-text-muted)">{{ $t('settings.system.waylandNote') }}</p>

      <UFormField>
        <template #label>
          <FieldLabel
            :label="$t('settings.system.textModeLabel')"
            :hint="$t('settings.system.textModeHint')"
          />
        </template>
        <URadioGroup
          v-model="textMode"
          :items="textModeItems"
          orientation="vertical"
          size="sm"
        />
      </UFormField>
    </div>
  </UCard>
</template>
