<script setup lang="ts">
// Bind `settings.appearance` to Nuxt UI's colorMode as soon as the app
// boots; the composable handles the 'system' case automatically.
useAppTheme()
</script>

<template>
  <UApp>
    <NuxtPage />
  </UApp>
</template>
