<script setup lang="ts">
import AppShell from '~/components/app/AppShell.vue'

const { loaded, load } = useConfig()
const route = useRoute()

useAppTheme()
useAppLocale()
useLayoutSwitcher()

onMounted(() => {
  if (!loaded.value) {
    void load()
  }
})
</script>

<template>
  <UApp>
    <NuxtPage v-if="route.path === '/quick-menu'" />
    <AppShell v-else>
      <NuxtPage />
    </AppShell>
  </UApp>
</template>
