<script setup lang="ts">
import AppHeader from '~/components/app/AppHeader.vue'
import LayoutsTab from '~/components/LayoutsTab.vue'
import RulesTab from '~/components/RulesTab.vue'
import KeymapTab from '~/components/KeymapTab.vue'
import MacrosTab from '~/components/MacrosTab.vue'
import SettingsTab from '~/components/SettingsTab.vue'
import { UI_CONSTANTS } from '~/utils/constants'

const { loaded } = useConfig()
const active = ref<string>('layouts')

const mainRef = ref<HTMLElement | null>(null)
const isScrolled = ref(false)

function onScroll() {
  if (!mainRef.value) return
  isScrolled.value = mainRef.value.scrollTop > UI_CONSTANTS.SCROLL_THRESHOLD
}

function scrollToTop() {
  mainRef.value?.scrollTo({ top: 0, behavior: 'smooth' })
}

watch(active, () => {
  onScroll()
})
</script>

<template>
  <div class="h-screen flex flex-col overflow-hidden relative">
    <AppHeader v-model:active-tab="active" />

    <main ref="mainRef" class="flex-1 overflow-y-auto p-4" @scroll="onScroll">
      <div>
        <LayoutsTab v-if="loaded && active === 'layouts'" />
        <RulesTab
          v-else-if="loaded && active === 'rules'"
          :show-back-to-top="isScrolled"
          @back-to-top="scrollToTop"
        />
        <KeymapTab v-else-if="loaded && active === 'keymap'" />
        <MacrosTab v-else-if="loaded && active === 'macros'" @back-to-top="scrollToTop" />
        <SettingsTab v-else-if="loaded && active === 'settings'" />
      </div>
    </main>
  </div>
</template>
