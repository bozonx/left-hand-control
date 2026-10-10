<script setup lang="ts">
import AppShell from '~/components/app/AppShell.vue'

const { loaded, load } = useConfig()
const route = useRoute()

useAppTheme()
useAppLocale()
useLayoutSwitcher()

onMounted(() => {
  // Marks "running inside the Tauri shell" so CSS can opt the page background
  // into transparency (borderless window → rounded corners + shadow).
  if ('__TAURI_INTERNALS__' in window) {
    document.documentElement.classList.add('is-tauri')
  }
  if (!loaded.value) {
    void load()
  }
})
</script>

<template>
  <UApp>
    <NuxtPage v-if="route.path === '/quick-menu' || route.path === '/emoji-menu'" />
    <AppShell v-else>
      <NuxtPage />
    </AppShell>
  </UApp>
</template>
