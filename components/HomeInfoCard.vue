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
        <div class="font-medium">
          {{ $t("settings.outputPrincipleTitle") }}
        </div>
        <p class="mt-1 text-(--ui-text-muted)">
          {{ $t("settings.outputPrincipleBody1") }}
        </p>
        <p class="mt-2 text-(--ui-text-muted)">
          {{ $t("settings.outputPrincipleBody2") }}
        </p>
      </div>

      <div class="rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/20 p-3">
        <div class="flex items-start gap-2">
          <UIcon name="i-lucide-folder-open" class="mt-0.5 shrink-0 text-(--ui-primary)" />
          <div>
            <div class="font-medium">{{ $t("home.layoutsHintTitle") }}</div>
            <p class="mt-1 text-(--ui-text-muted)">
              {{ $t("home.layoutsHintBody") }}
            </p>
          </div>
        </div>
      </div>
    </div>
  </UCard>
</template>
