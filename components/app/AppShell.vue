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
  <div class="min-h-screen flex flex-col">
    <AppHeader />

    <main class="flex-1 p-4 w-full">
      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-elevated) p-1 inline-flex gap-1 app-chrome">
        <UButton
          v-for="item in tabItems"
          :key="item.value"
          :color="active === item.value ? 'primary' : 'neutral'"
          :variant="active === item.value ? 'soft' : 'ghost'"
          :icon="item.icon"
          @click="active = item.value"
        >
          {{ item.label }}
        </UButton>
      </div>

      <div class="mt-4">
        <RulesTab v-if="loaded && active === 'rules'" />
        <KeymapTab v-else-if="loaded && active === 'keymap'" />
        <MacrosTab v-else-if="loaded && active === 'macros'" />
        <SettingsTab v-else-if="loaded && active === 'settings'" />
      </div>
    </main>
  </div>
</template>
