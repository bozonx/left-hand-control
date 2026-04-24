<script setup lang="ts">
import AppHeader from '~/components/app/AppHeader.vue'

const { loaded, loadError } = useConfig()

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
  <div class="h-screen flex flex-col overflow-hidden relative">
    <AppHeader />

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

        <UCard v-if="!loaded && !loadError">
          <div class="py-12 text-center">
            <h2 class="text-sm font-semibold">{{ $t('app.title') }}</h2>
            <p class="mt-1 text-sm text-(--ui-text-muted)">
              {{ $t('app.loading') }}
            </p>
          </div>
        </UCard>

        <slot v-else />
      </div>
    </main>
  </div>
</template>
