<script setup lang="ts">
import CommandsCard from '~/components/features/settings/CommandsCard.vue'
import MapperCard from '~/components/features/settings/MapperCard.vue'
import GeneralCard from '~/components/features/settings/GeneralCard.vue'
import BehaviorCard from '~/components/features/settings/BehaviorCard.vue'
import GameModeCard from '~/components/features/settings/GameModeCard.vue'
import SystemCard from '~/components/features/settings/SystemCard.vue'
import ConfigPathCard from '~/components/features/settings/ConfigPathCard.vue'
const {
  config,
  settingsDir,
  library,
  mapper,
  mapperIssues,
  platform,
  theme,
  appLocale,
  appearanceItems,
  localeItems,
  deviceOptions,
  selectedDevice,
  mouseOptions,
  selectedMouse,
  toggleMapper,
} = useSettingsScreen()

const tab = ref('general')
const { t } = useI18n()
const tabs = computed(() => [{ label: t('settings.generalTitle'), value: 'general' }, { label: t('commands.title'), value: 'commands' }])

function updateConfig(nextConfig: typeof config.value) {
  config.value = nextConfig
}
</script>

<template>
  <div data-testid="settings-page" class="mx-auto w-full max-w-5xl space-y-4">
    <UTabs v-model="tab" :items="tabs" :content="false" />
    <CommandsCard v-if="tab === 'commands'" />
    <div v-else class="space-y-4">
    <MapperCard
      v-model:selected-device="selectedDevice"
      v-model:selected-mouse="selectedMouse"
      :mapper="mapper"
      :device-options="deviceOptions"
      :mouse-options="mouseOptions"
      :issues="mapperIssues"
      @toggle="toggleMapper"
    />

    <GeneralCard
      v-model:theme-preference="theme.preference.value"
      v-model:locale-preference="appLocale.preference.value"
      :config="config"
      :resolved-theme="theme.resolved.value"
      :appearance-items="appearanceItems"
      :locale-items="localeItems"
    />

    <BehaviorCard :config="config" @update:config="updateConfig" />

    <GameModeCard
      :use-gamemoded="config.settings.gameMode?.useGamemoded ?? true"
      :use-fullscreen="config.settings.gameMode?.useFullscreen ?? false"
      :process-matchers="config.settings.gameMode?.processMatchers ?? []"
      @update:use-gamemoded="(v) => { config.settings.gameMode.useGamemoded = v }"
      @update:use-fullscreen="(v) => { config.settings.gameMode.useFullscreen = v }"
      @update:process-matchers="(v) => { config.settings.gameMode.processMatchers = v }"
    />

    <SystemCard :platform="platform.info.value" />

    <ConfigPathCard
      :settings-dir="settingsDir"
      :layouts-dir="library.layoutsDir.value"
    />
    </div>
  </div>
</template>
