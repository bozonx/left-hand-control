<script setup lang="ts">
import MapperCard from '~/components/features/settings/MapperCard.vue'
import GeneralCard from '~/components/features/settings/GeneralCard.vue'
import BehaviorCard from '~/components/features/settings/BehaviorCard.vue'
import GameModeCard from '~/components/features/settings/GameModeCard.vue'
import ConfigPathCard from '~/components/features/settings/ConfigPathCard.vue'
const {
  config,
  settingsDir,
  library,
  mapper,
  mapperIssues,
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
</script>

<template>
  <div class="mx-auto w-full max-w-5xl space-y-4">
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
      :config="config"
      v-model:theme-preference="theme.preference.value"
      :resolved-theme="theme.resolved.value"
      v-model:locale-preference="appLocale.preference.value"
      :appearance-items="appearanceItems"
      :locale-items="localeItems"
    />

    <BehaviorCard :config="config" />

    <GameModeCard v-model:game-mode="config.value.settings.gameMode" />

    <ConfigPathCard
      :settings-dir="settingsDir"
      :layouts-dir="library.layoutsDir.value"
    />
  </div>
</template>
