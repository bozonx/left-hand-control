<script setup lang="ts">
import RulesTab from '~/components/RulesTab.vue'
import { useAppShellScroll } from '~/composables/useAppShellScroll'

const { isScrolled, scrollToTop } = useAppShellScroll()
</script>

<template>
  <RulesTab :show-back-to-top="isScrolled" @back-to-top="scrollToTop" />
</template>
