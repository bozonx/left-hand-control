<script setup lang="ts">
import MacrosTab from '~/components/MacrosTab.vue'
import { useAppShellScroll } from '~/composables/useAppShellScroll'

const { scrollToTop } = useAppShellScroll()
</script>

<template>
  <MacrosTab @back-to-top="scrollToTop" />
</template>
