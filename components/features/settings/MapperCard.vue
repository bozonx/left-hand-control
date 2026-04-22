<script setup lang="ts">
import type { MapperState } from '~/composables/useMapper'

defineProps<{
  mapper: MapperState
  deviceOptions: Array<{ label: string, value: string }>
  selectedDevice: string
}>()

defineEmits<{
  'update:selectedDevice': [value: string]
  toggle: []
}>()
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <h2 class="text-sm font-semibold">{{ $t('settings.mapperTitle') }}</h2>
        <UBadge
          :color="mapper.status.value.running ? 'success' : 'neutral'"
          variant="subtle"
        >
          {{ mapper.status.value.running ? $t('common.active') : $t('common.stopped') }}
        </UBadge>
      </div>
    </template>
    <div class="space-y-4">
      <UFormField
        :label="$t('settings.keyboardLabel')"
        :help="$t('settings.keyboardHelp')"
      >
        <div class="flex gap-2 items-center">
          <USelectMenu
            :model-value="selectedDevice"
            :items="deviceOptions"
            value-key="value"
            :placeholder="$t('settings.devicePh')"
            class="flex-1"
            :disabled="mapper.status.value.running"
            @update:model-value="(value: string) => $emit('update:selectedDevice', value)"
          />
          <UButton
            variant="ghost"
            icon="i-lucide-refresh-cw"
            :aria-label="$t('settings.refreshDevices')"
            :disabled="mapper.status.value.running"
            @click="mapper.refreshDevices()"
          />
        </div>
      </UFormField>

      <div class="flex items-center gap-2">
        <UButton
          :color="mapper.status.value.running ? 'error' : 'primary'"
          :icon="
            mapper.status.value.running
              ? 'i-lucide-square'
              : 'i-lucide-play'
          "
          :loading="mapper.busy.value"
          :disabled="!mapper.status.value.running && !selectedDevice"
          @click="$emit('toggle')"
        >
          {{ mapper.status.value.running ? $t('settings.stop') : $t('settings.start') }}
        </UButton>
        <span
          v-if="mapper.error.value"
          class="text-sm text-(--ui-error) break-all"
        >
          {{ mapper.error.value }}
        </span>
      </div>

      <p class="text-xs text-(--ui-text-muted)">
        <i18n-t keypath="settings.mapperHint" tag="span">
          <template #input><code>/dev/input/eventX</code></template>
          <template #uinput><code>uinput</code></template>
          <template #group><code>input</code></template>
          <template #uinputDev><code>/dev/uinput</code></template>
        </i18n-t>
      </p>
    </div>
  </UCard>
</template>
