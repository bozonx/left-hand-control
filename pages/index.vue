<script setup lang="ts">
import AppShell from '~/components/app/AppShell.vue'

const {
  loaded,
  needsWelcome,
  load,
} = useConfig()

onMounted(() => {
  void load()
})
</script>

<template>
  <WelcomeScreen v-if="loaded && needsWelcome" />
  <AppShell v-else />
</template>
