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
  settingsBanner,
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
    <UAlert
      v-if="settingsBanner"
      :color="settingsBanner.color"
      variant="soft"
      :icon="settingsBanner.icon"
      :title="settingsBanner.title"
    >
      <template #description>
        <ul class="space-y-2 text-sm">
          <li
            v-for="issue in settingsBanner.issues"
            :key="issue.id"
          >
            <span class="font-medium">{{ issue.title }}</span>
            <span class="text-(--ui-text-muted)"> — {{ issue.description }}</span>
          </li>
        </ul>
      </template>
    </UAlert>

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

    <GameModeCard :config="config" />

    <ConfigPathCard
      :settings-dir="settingsDir"
      :layouts-dir="library.layoutsDir.value"
    />
  </div>
</template>
