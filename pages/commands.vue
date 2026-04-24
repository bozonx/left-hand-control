<script setup lang="ts">
import CommandsTab from '~/components/CommandsTab.vue'
import { useAppShellScroll } from '~/composables/useAppShellScroll'

const { scrollToTop } = useAppShellScroll()
</script>

<template>
  <CommandsTab @back-to-top="scrollToTop" />
</template>
