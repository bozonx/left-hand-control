<script setup lang="ts">
import AppShell from '~/components/app/AppShell.vue'
import LoadingScreen from '~/components/LoadingScreen.vue'

const {
  loaded,
  loadError,
  load,
} = useConfig()

onMounted(() => {
  void load()
})

function retryLoad() {
  void load()
}
</script>

<template>
  <LoadingScreen
    v-if="!loaded || loadError"
    :error="loadError"
    @retry="retryLoad"
  />
  <AppShell v-else />
</template>
