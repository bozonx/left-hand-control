<template>
  <UCard class="h-full">
    <template #header>
      <div class="flex items-center gap-2">
        <UIcon name="i-lucide-info" class="shrink-0 text-(--ui-primary)" />
        <div>
          <h2 class="text-sm font-semibold">{{ $t("home.infoTitle") }}</h2>
          <p class="text-sm text-(--ui-text-muted)">
            {{ $t("home.infoSubtitle") }}
          </p>
        </div>
      </div>
    </template>

    <div class="space-y-3 text-sm">
      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/40 p-3">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-zap" class="mt-0.5 shrink-0 text-(--ui-primary)" />
          <div>
            <div class="font-medium">{{ $t("home.whatIsItTitle") }}</div>
            <p class="mt-1 text-(--ui-text-muted)">
              {{ $t("home.whatIsItBody") }}
            </p>
          </div>
        </div>
      </div>

      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/40 p-3">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-power" class="mt-0.5 shrink-0 text-(--ui-primary)" />
          <div>
            <div class="font-medium">{{ $t("home.howToStartTitle") }}</div>
            <p class="mt-1 text-(--ui-text-muted)">
              {{ $t("home.howToStartBody") }}
            </p>
          </div>
        </div>
      </div>

      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/40 p-3">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-layers" class="mt-0.5 shrink-0 text-(--ui-primary)" />
          <div>
            <div class="font-medium">{{ $t("home.layersExplainedTitle") }}</div>
            <p class="mt-1 text-(--ui-text-muted)">
              {{ $t("home.layersExplainedBody") }}
            </p>
          </div>
        </div>
      </div>

      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/40 p-3">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-mouse-pointer-click" class="mt-0.5 shrink-0 text-(--ui-primary)" />
          <div>
            <div class="font-medium">{{ $t("home.rulesExplainedTitle") }}</div>
            <p class="mt-1 text-(--ui-text-muted)">
              {{ $t("home.rulesExplainedBody") }}
            </p>
          </div>
        </div>
      </div>

      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/20 p-3">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-rocket" class="mt-0.5 shrink-0 text-(--ui-primary)" />
          <div>
            <div class="font-medium">{{ $t("home.builtInLayoutTitle") }}</div>
            <p class="mt-1 text-(--ui-text-muted)">
              {{ $t("home.builtInLayoutBody") }}
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
          {{ $t("home.fullDocsLabel") }}
        </UButton>
      </div>
    </div>
  </UCard>
</template>
