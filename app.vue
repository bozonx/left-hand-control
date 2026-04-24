<script setup lang="ts">
import AppShell from '~/components/app/AppShell.vue'

const { loaded, load } = useConfig()

useAppTheme()
useAppLocale()

onMounted(() => {
  if (!loaded.value) {
    void load()
  }
})
</script>

<template>
  <UApp>
    <AppShell>
      <NuxtPage />
    </AppShell>
  </UApp>
</template>
