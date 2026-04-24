<script setup lang="ts">
import MapperCard from '~/components/features/settings/MapperCard.vue'
import GeneralCard from '~/components/features/settings/GeneralCard.vue'
import BehaviorCard from '~/components/features/settings/BehaviorCard.vue'
import ConfigPathCard from '~/components/features/settings/ConfigPathCard.vue'
import PlatformCard from '~/components/features/settings/PlatformCard.vue'
const {
  config,
  configPath,
  library,
  mapper,
  platform,
  theme,
  appLocale,
  appearanceItems,
  localeItems,
  deviceOptions,
  selectedDevice,
  toggleMapper,
} = useSettingsScreen()
</script>

<template>
  <div class="mx-auto w-full max-w-5xl space-y-4">
    <MapperCard
      v-model:selected-device="selectedDevice"
      :mapper="mapper"
      :device-options="deviceOptions"
      @toggle="toggleMapper"
    />

    <GeneralCard
      :config="config"
      v-model:theme-preference="theme.preference.value"
      :resolved-theme="theme.resolved.value"
      v-model:locale-preference="appLocale.preference.value"
      :appearance-items="appearanceItems"
      :locale-items="localeItems"
    />

    <BehaviorCard :config="config" />

    <PlatformCard :platform="platform" />

    <ConfigPathCard
      :config-path="configPath"
      :layouts-dir="library.layoutsDir.value"
    />
  </div>
</template>
