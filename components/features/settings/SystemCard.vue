<script setup lang="ts">
import type { AppConfig, LinuxWaylandTextMode } from '~/types/config'
import type { PlatformInfo } from '~/types/platform'

const props = defineProps<{
  config: AppConfig
  platform: PlatformInfo | null
}>()

const emit = defineEmits<{
  'update:config': [config: AppConfig]
}>()

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
  get: () => props.config.settings.linuxWaylandTextMode ?? 'keycode',
  set: (value: LinuxWaylandTextMode) => {
    emit('update:config', {
      ...props.config,
      settings: {
        ...props.config.settings,
        linuxWaylandTextMode: value,
      },
    })
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
          value-key="value"
        />
      </UFormField>
    </div>
  </UCard>
</template>
