<script setup lang="ts">
import AppHeader from '~/components/app/AppHeader.vue'
import WindowResizeHandles from '~/components/app/WindowResizeHandles.vue'
import WelcomeScreen from '~/components/WelcomeScreen.vue'
import LayoutModals from '~/components/features/settings/LayoutModals.vue'

const { loaded, loadError, needsWelcome } = useConfig()

// Borderless + transparent window: round the app's corners (the area outside
// the radius is transparent → shows the desktop). Skipped when maximized
// (fills the screen, square corners) and outside the Tauri shell (a plain
// browser has no window chrome to mimic).
const { available: hasWindowChrome, isMaximized } = useWindowControls()
const floating = computed(() => hasWindowChrome.value && !isMaximized.value)

const route = useRoute()
const isFullWidth = computed(() => route.meta.fullWidth === true)

const mainRef = ref<HTMLElement | null>(null)
const isScrolled = ref(false)

function onScroll() {
  if (!mainRef.value) return
  isScrolled.value = mainRef.value.scrollTop > 48
}

function scrollToTop() {
  mainRef.value?.scrollTo({ top: 0, behavior: 'smooth' })
}

provide('app-shell-scroll', {
  isScrolled: readonly(isScrolled),
  scrollToTop,
})
</script>

<template>
  <WelcomeScreen v-if="loaded && !loadError && needsWelcome" />

  <WindowResizeHandles />

  <div
    class="h-screen flex flex-col overflow-hidden relative bg-(--ui-bg)"
    :class="floating ? 'rounded-xl border border-(--ui-border)' : ''"
  >
    <AppHeader />
    <LayoutModals />

    <main ref="mainRef" class="flex-1 overflow-y-auto p-4" @scroll="onScroll">
      <div
        class="w-full space-y-4"
        :class="isFullWidth ? '' : 'mx-auto max-w-7xl'"
      >
        <UAlert
          v-if="loadError"
          color="error"
          variant="soft"
          icon="i-lucide-circle-alert"
          :title="$t('app.loadFailedBody')"
          :description="loadError"
        />

        <UCard v-show="!loaded && !loadError">
          <div class="py-12 text-center">
            <h2 class="text-sm font-semibold">{{ $t('app.title') }}</h2>
            <p class="mt-1 text-sm text-(--ui-text-muted)">
              {{ $t('app.loading') }}
            </p>
          </div>
        </UCard>

        <div v-show="loaded && !loadError" class="contents">
          <slot />
        </div>
      </div>
    </main>
  </div>
</template>
