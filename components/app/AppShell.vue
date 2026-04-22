<script setup lang="ts">
import AppHeader from '~/components/app/AppHeader.vue'
import RulesTab from '~/components/RulesTab.vue'
import KeymapTab from '~/components/KeymapTab.vue'
import MacrosTab from '~/components/MacrosTab.vue'
import SettingsTab from '~/components/SettingsTab.vue'
import { UI_CONSTANTS } from '~/utils/constants'
import { useRulesEditor } from '~/composables/useRulesEditor'

const { loaded } = useConfig()
const active = ref<string>('rules')

const mainRef = ref<HTMLElement | null>(null)
const isScrolled = ref(false)

function onScroll() {
  if (!mainRef.value) return
  isScrolled.value = mainRef.value.scrollTop > UI_CONSTANTS.SCROLL_THRESHOLD
}

function scrollToTop() {
  mainRef.value?.scrollTo({ top: 0, behavior: 'smooth' })
}

const { addRule } = useRulesEditor()

watch(active, () => {
  onScroll()
})
</script>

<template>
  <div class="h-screen flex flex-col overflow-hidden relative">
    <AppHeader v-model:active-tab="active" />

    <main ref="mainRef" class="flex-1 overflow-y-auto p-4" @scroll="onScroll">
      <div>
        <RulesTab v-if="loaded && active === 'rules'" />
        <KeymapTab v-else-if="loaded && active === 'keymap'" />
        <MacrosTab v-else-if="loaded && active === 'macros'" />
        <SettingsTab v-else-if="loaded && active === 'settings'" />
      </div>
    </main>

    <!-- Floating Actions for Rules Tab -->
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="translate-y-10 opacity-0"
      enter-to-class="translate-y-0 opacity-100"
      leave-active-class="transition duration-200 ease-in"
      leave-from-class="translate-y-0 opacity-100"
      leave-to-class="translate-y-10 opacity-0"
    >
      <div
        v-if="active === 'rules' && isScrolled"
        class="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 pointer-events-none"
      >
        <div class="flex flex-col items-center gap-2 pointer-events-auto">
          <UButton
            icon="i-lucide-plus"
            size="lg"
            class="shadow-xl rounded-full px-6"
            @click="addRule"
          >
            {{ $t('rules.addBtn') }}
          </UButton>

          <ULink
            class="text-xs text-(--ui-text-muted) hover:text-(--ui-primary) transition-colors cursor-pointer"
            @click="scrollToTop"
          >
            {{ $t('common.backToTop') }}
          </ULink>
        </div>
      </div>
    </Transition>
  </div>
</template>

