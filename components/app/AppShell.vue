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
  { value: 'rules', label: t('tabs.rules'), icon: 'i-lucide-layers' },
  { value: 'keymap', label: t('tabs.keymap'), icon: 'i-lucide-keyboard' },
  { value: 'macros', label: t('tabs.macros'), icon: 'i-lucide-zap' },
  { value: 'settings', label: t('tabs.settings'), icon: 'i-lucide-settings' },
])
</script>

<template>
  <div class="h-screen flex flex-col overflow-hidden">
    <AppHeader />

    <main class="flex-1 overflow-y-auto p-4">
      <div class="rounded-md border border-(--ui-border) bg-(--ui-bg-elevated) p-0.5 inline-flex gap-0.5 app-chrome mb-4">
        <UButton
          v-for="item in tabItems"
          :key="item.value"
          :color="active === item.value ? 'primary' : 'neutral'"
          :variant="active === item.value ? 'soft' : 'ghost'"
          :icon="item.icon"
          size="sm"
          @click="active = item.value"
        >
          {{ item.label }}
        </UButton>
      </div>

      <div>
        <RulesTab v-if="loaded && active === 'rules'" />
        <KeymapTab v-else-if="loaded && active === 'keymap'" />
        <MacrosTab v-else-if="loaded && active === 'macros'" />
        <SettingsTab v-else-if="loaded && active === 'settings'" />
      </div>
    </main>
  </div>
</template>
