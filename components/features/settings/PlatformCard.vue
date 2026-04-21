<script setup lang="ts">
import type { PlatformInfoState } from '~/composables/usePlatformInfo'
import type { CapabilityStatus } from '~/types/platform'

const props = defineProps<{
  platform: PlatformInfoState
}>()

const capabilityItems = computed(() => {
  const info = props.platform.info.value
  if (!info) return []

  return [
    {
      key: 'key_interception',
      label: 'settings.platformCapabilities.keyInterception',
      status: info.capabilities.key_interception,
    },
    {
      key: 'literal_injection',
      label: 'settings.platformCapabilities.literalInjection',
      status: info.capabilities.literal_injection,
    },
    {
      key: 'layout_detection',
      label: 'settings.platformCapabilities.layoutDetection',
      status: info.capabilities.layout_detection,
    },
    {
      key: 'system_actions',
      label: 'settings.platformCapabilities.systemActions',
      status: info.capabilities.system_actions,
    },
  ]
})

function supportColor(status: CapabilityStatus) {
  return status.supported ? 'primary' : 'neutral'
}

function availabilityColor(status: CapabilityStatus) {
  return status.available ? 'success' : 'warning'
}
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <div>
          <h2 class="font-semibold">{{ $t('settings.platformTitle') }}</h2>
          <p class="text-xs text-(--ui-text-muted)">
            {{ $t('settings.platformHint') }}
          </p>
        </div>
        <UButton
          variant="ghost"
          icon="i-lucide-refresh-cw"
          :loading="platform.busy.value"
          :label="$t('common.refresh')"
          @click="platform.refresh()"
        />
      </div>
    </template>

    <div class="space-y-4">
      <p v-if="platform.error.value" class="text-sm text-(--ui-error)">
        {{ platform.error.value }}
      </p>

      <template v-else-if="platform.info.value">
        <div class="grid gap-3 md:grid-cols-2">
          <div class="rounded-lg border border-(--ui-border) p-3">
            <div class="text-xs uppercase tracking-wide text-(--ui-text-muted)">
              {{ $t('settings.platformOs') }}
            </div>
            <div class="mt-1 font-medium">
              {{ platform.info.value.os }}
            </div>
          </div>
          <div v-if="platform.info.value.linux" class="rounded-lg border border-(--ui-border) p-3">
            <div class="text-xs uppercase tracking-wide text-(--ui-text-muted)">
              {{ $t('settings.platformSession') }}
            </div>
            <div class="mt-1 font-medium">
              {{ platform.info.value.linux.desktop }} / {{ platform.info.value.linux.session_type }}
            </div>
            <div class="mt-1 text-xs text-(--ui-text-muted)">
              XDG: <code>{{ platform.info.value.linux.xdg_current_desktop || '—' }}</code>
            </div>
          </div>
        </div>

        <div class="space-y-3">
          <div
            v-for="item in capabilityItems"
            :key="item.key"
            class="rounded-lg border border-(--ui-border) p-3"
          >
            <div class="flex items-start justify-between gap-3">
              <div>
                <div class="font-medium">{{ $t(item.label) }}</div>
                <div v-if="item.status.detail" class="mt-1 text-xs text-(--ui-text-muted)">
                  {{ item.status.detail }}
                </div>
              </div>
              <div class="flex shrink-0 gap-2">
                <UBadge :color="supportColor(item.status)" variant="subtle">
                  {{ $t(item.status.supported ? 'settings.supportedYes' : 'settings.supportedNo') }}
                </UBadge>
                <UBadge :color="availabilityColor(item.status)" variant="subtle">
                  {{ $t(item.status.available ? 'settings.availableNow' : 'settings.unavailableNow') }}
                </UBadge>
              </div>
            </div>
          </div>
        </div>
      </template>

      <p v-else class="text-sm text-(--ui-text-muted)">
        {{ $t('settings.platformUnavailable') }}
      </p>
    </div>
  </UCard>
</template>
