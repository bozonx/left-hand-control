<script setup lang="ts">
import AppHeader from '~/components/app/AppHeader.vue'
import RulesTab from '~/components/RulesTab.vue'
import KeymapTab from '~/components/KeymapTab.vue'
import MacrosTab from '~/components/MacrosTab.vue'
import SettingsTab from '~/components/SettingsTab.vue'

const { loaded } = useConfig()

const active = ref<string>('rules')
</script>

<template>
  <div class="h-screen flex flex-col overflow-hidden">
    <AppHeader v-model:active-tab="active" />

    <main class="flex-1 overflow-y-auto p-4">
      <div>
        <RulesTab v-if="loaded && active === 'rules'" />
        <KeymapTab v-else-if="loaded && active === 'keymap'" />
        <MacrosTab v-else-if="loaded && active === 'macros'" />
        <SettingsTab v-else-if="loaded && active === 'settings'" />
      </div>
    </main>
  </div>
</template>

