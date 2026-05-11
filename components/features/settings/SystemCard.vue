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
    value: 'libei' as LinuxWaylandTextMode,
    label: t('settings.system.textModeLibei'),
    description: t('settings.system.textModeLibeiHint'),
  },
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
  {
    value: 'ydotool' as LinuxWaylandTextMode,
    label: t('settings.system.textModeYdotool'),
    description: t('settings.system.textModeYdotoolHint'),
  },
  {
    value: 'xdotool' as LinuxWaylandTextMode,
    label: t('settings.system.textModeXdotool'),
    description: t('settings.system.textModeXdotoolHint'),
  },
])

const textMode = computed({
  get: () => config.value.settings.linuxWaylandTextMode ?? 'libei',
  set: (value: LinuxWaylandTextMode) => {
    config.value.settings.linuxWaylandTextMode = value
  },
})

const ydotoolPath = computed({
  get: () => config.value.settings.linuxYdotoolPath ?? '',
  set: (value: string) => {
    config.value.settings.linuxYdotoolPath = value
  },
})

const xdotoolPath = computed({
  get: () => config.value.settings.linuxXdotoolPath ?? '',
  set: (value: string) => {
    config.value.settings.linuxXdotoolPath = value
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

      <UFormField v-if="textMode === 'ydotool'">
        <template #label>
          <FieldLabel
            :label="$t('settings.system.ydotoolPathLabel')"
            :hint="$t('settings.system.ydotoolPathHint')"
          />
        </template>
        <UInput
          v-model="ydotoolPath"
          :placeholder="$t('settings.system.ydotoolPathPlaceholder')"
          class="w-full"
        />
      </UFormField>

      <UFormField v-if="textMode === 'xdotool'">
        <template #label>
          <FieldLabel
            :label="$t('settings.system.xdotoolPathLabel')"
            :hint="$t('settings.system.xdotoolPathHint')"
          />
        </template>
        <UInput
          v-model="xdotoolPath"
          :placeholder="$t('settings.system.xdotoolPathPlaceholder')"
          class="w-full"
        />
      </UFormField>
    </div>
  </UCard>
</template>
