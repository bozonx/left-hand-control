<script setup lang="ts">
const platform = usePlatformInfo()
const { t } = useI18n()

onMounted(() => {
  platform.refresh()
})

const platformLabel = computed(() => {
  const info = platform.info.value
  if (!info) return null
  if (info.linux) {
    return t('home.platformDetected', { platform: `Linux / ${info.linux.desktop}` })
  }
  return t('home.platformDetected', { platform: info.os })
})

interface FeatureRow {
  label: string
  body: string
  supported: boolean
  available: boolean
}

const features = computed<FeatureRow[]>(() => {
  const info = platform.info.value
  if (!info) return []

  const caps = info.capabilities
  const rows: FeatureRow[] = []

  if (info.os === 'linux' && info.linux) {
    rows.push({
      label: t('home.keyInterceptionLabel'),
      body: t('home.keyInterceptionLinux'),
      supported: caps.key_interception.supported,
      available: caps.key_interception.available,
    })
    rows.push({
      label: t('home.textInjectionLabel'),
      body: t('home.textInjectionLinux'),
      supported: caps.literal_injection.supported,
      available: caps.literal_injection.available,
    })
    const desktop = info.linux.desktop
    if (desktop === 'KDE Plasma') {
      rows.push({
        label: t('home.systemActionsLabel'),
        body: t('home.systemActionsLinuxKde'),
        supported: caps.system_actions.supported,
        available: caps.system_actions.available,
      })
      rows.push({
        label: t('home.layoutDetectionLabel'),
        body: t('home.layoutDetectionLinuxKde'),
        supported: caps.layout_detection.supported,
        available: caps.layout_detection.available,
      })
    } else {
      rows.push({
        label: t('home.systemActionsLabel'),
        body: t('home.systemActionsLinuxOther', { desktop }),
        supported: caps.system_actions.supported,
        available: caps.system_actions.available,
      })
      rows.push({
        label: t('home.layoutDetectionLabel'),
        body: t('home.layoutDetectionLinuxOther', { desktop }),
        supported: caps.layout_detection.supported,
        available: caps.layout_detection.available,
      })
    }
  } else {
    rows.push({
      label: t('home.keyInterceptionLabel'),
      body: t('home.keyInterceptionStub'),
      supported: false,
      available: false,
    })
    rows.push({
      label: t('home.textInjectionLabel'),
      body: t('home.textInjectionStub'),
      supported: false,
      available: false,
    })
    rows.push({
      label: t('home.systemActionsLabel'),
      body: t('home.systemActionsStub'),
      supported: false,
      available: false,
    })
    rows.push({
      label: t('home.layoutDetectionLabel'),
      body: t('home.layoutDetectionStub'),
      supported: false,
      available: false,
    })
  }

  return rows
})
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center gap-2">
        <UIcon name="i-lucide-cpu" class="shrink-0 text-(--ui-primary)" />
        <div>
          <h2 class="text-sm font-semibold">{{ $t('home.howItWorksTitle') }}</h2>
        </div>
      </div>
    </template>

    <div class="space-y-3 text-sm">
      <p v-if="platformLabel" class="text-xs text-(--ui-text-muted)">
        {{ platformLabel }}
      </p>

      <div v-if="platform.busy.value" class="text-xs text-(--ui-text-muted)">
        {{ $t('app.loading') }}
      </div>

      <div v-else-if="platform.error.value" class="text-xs text-(--ui-error)">
        {{ platform.error.value }}
      </div>

      <div
        v-for="feat in features"
        :key="feat.label"
        class="space-y-1"
      >
        <div class="flex items-center gap-2 flex-wrap">
          <span class="font-medium text-xs">{{ feat.label }}</span>
          <UBadge
            v-if="feat.supported && feat.available"
            color="success"
            variant="subtle"
            size="sm"
          >
            {{ $t('home.statusAvailable') }}
          </UBadge>
          <UBadge
            v-else-if="feat.supported && !feat.available"
            color="warning"
            variant="subtle"
            size="sm"
          >
            {{ $t('home.statusUnavailable') }}
          </UBadge>
          <UBadge
            v-else
            color="neutral"
            variant="subtle"
            size="sm"
          >
            {{ $t('home.statusSupported') }}
          </UBadge>
        </div>
        <p class="text-xs text-(--ui-text-muted)">
          {{ feat.body }}
        </p>
      </div>
    </div>
  </UCard>
</template>
