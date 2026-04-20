<script setup lang="ts">
import AppHeader from '~/components/app/AppHeader.vue'
import RulesTab from '~/components/RulesTab.vue'
import KeymapTab from '~/components/KeymapTab.vue'
import MacrosTab from '~/components/MacrosTab.vue'
import SettingsTab from '~/components/SettingsTab.vue'

const { loaded } = useConfig()
const { t } = useI18n()

const active = ref<string>('rules')

const tabItems = computed(() => [
  { value: 'rules', slot: 'rules', label: t('tabs.rules'), icon: 'i-lucide-layers' },
  { value: 'keymap', slot: 'keymap', label: t('tabs.keymap'), icon: 'i-lucide-keyboard' },
  { value: 'macros', slot: 'macros', label: t('tabs.macros'), icon: 'i-lucide-zap' },
  { value: 'settings', slot: 'settings', label: t('tabs.settings'), icon: 'i-lucide-settings' },
])
</script>

<template>
  <div class="min-h-screen flex flex-col">
    <AppHeader />

    <main class="flex-1 p-4 w-full">
      <UTabs
        v-model="active"
        :items="tabItems"
        class="w-full"
      >
        <template #rules>
          <div class="mt-4">
            <RulesTab v-if="loaded" />
          </div>
        </template>
        <template #keymap>
          <div class="mt-4">
            <KeymapTab v-if="loaded" />
          </div>
        </template>
        <template #macros>
          <div class="mt-4">
            <MacrosTab v-if="loaded" />
          </div>
        </template>
        <template #settings>
          <div class="mt-4">
            <SettingsTab v-if="loaded" />
          </div>
        </template>
      </UTabs>
    </main>
  </div>
</template>
