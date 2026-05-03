<script setup lang="ts">
const { t: _t } = useI18n()
const uiState = useUiState()
const isOpen = computed({
  get: () => uiState.state.value.homeHelpOpen,
  set: (value: boolean) => uiState.setHomeHelpOpen(value),
})

function toggleOpen() {
  isOpen.value = !isOpen.value
}

onMounted(() => {
  if (!uiState.loaded.value) void uiState.load()
})
</script>

<template>
  <UCard class="h-full">
    <template #header>
      <button
        type="button"
        class="w-full flex items-center justify-between gap-3 text-left cursor-pointer"
        :aria-expanded="isOpen"
        @click="toggleOpen"
      >
        <div class="flex items-center gap-2 min-w-0">
          <UIcon name="i-lucide-info" class="shrink-0 text-(--ui-primary)" />
          <div class="min-w-0">
            <h2 class="text-sm font-semibold">{{ $t('home.infoTitle') }}</h2>
            <p class="text-sm text-(--ui-text-muted)">
              {{ $t('home.infoSubtitle') }}
            </p>
          </div>
        </div>
        <UIcon
          :name="isOpen ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
          class="shrink-0 text-(--ui-text-muted)"
          aria-hidden="true"
        />
      </button>
    </template>

    <div v-show="isOpen" class="space-y-3 text-sm">
      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/40 p-3">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-zap" class="mt-0.5 shrink-0 text-(--ui-primary)" />
          <div>
            <div class="font-medium">{{ $t('home.whatIsItTitle') }}</div>
            <p class="mt-1 text-(--ui-text-muted)">
              {{ $t('home.whatIsItBody') }}
            </p>
          </div>
        </div>
      </div>

      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/40 p-3">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-power" class="mt-0.5 shrink-0 text-(--ui-primary)" />
          <div>
            <div class="font-medium">{{ $t('home.howToStartTitle') }}</div>
            <p class="mt-1 text-(--ui-text-muted)">
              {{ $t('home.howToStartBody') }}
            </p>
          </div>
        </div>
      </div>

      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/40 p-3">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-layers" class="mt-0.5 shrink-0 text-(--ui-primary)" />
          <div>
            <div class="font-medium">{{ $t('home.layersExplainedTitle') }}</div>
            <p class="mt-1 text-(--ui-text-muted)">
              {{ $t('home.layersExplainedBody') }}
            </p>
          </div>
        </div>
      </div>

      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/40 p-3">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-mouse-pointer-click" class="mt-0.5 shrink-0 text-(--ui-primary)" />
          <div>
            <div class="font-medium">{{ $t('home.rulesExplainedTitle') }}</div>
            <p class="mt-1 text-(--ui-text-muted)">
              {{ $t('home.rulesExplainedBody') }}
            </p>
          </div>
        </div>
      </div>

      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/20 p-3">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-rocket" class="mt-0.5 shrink-0 text-(--ui-primary)" />
          <div>
            <div class="font-medium">{{ $t('home.builtInLayoutTitle') }}</div>
            <p class="mt-1 text-(--ui-text-muted)">
              {{ $t('home.builtInLayoutBody') }}
            </p>
          </div>
        </div>
      </div>

      <div class="flex items-center justify-end">
        <UButton
          variant="link"
          color="primary"
          :to="'https://github.com/bozonx/left-hand-control/blob/main/README.md'"
          target="_blank"
          trailing-icon="i-lucide-external-link"
        >
          {{ $t('home.fullDocsLabel') }}
        </UButton>
      </div>
    </div>
  </UCard>
</template>
