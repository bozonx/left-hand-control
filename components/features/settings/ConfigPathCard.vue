<script setup lang="ts">
defineProps<{
  configPath: string
}>()
</script>

<template>
  <UCard>
    <template #header>
      <h2 class="font-semibold">{{ $t('settings.configTitle') }}</h2>
    </template>
    <div class="text-sm">
      <div class="text-(--ui-text-muted) mb-1">{{ $t('settings.configPath') }}</div>
      <code class="block p-2 rounded bg-(--ui-bg-muted) break-all">
        {{ configPath || '…' }}
      </code>
      <p class="text-xs text-(--ui-text-muted) mt-2">
        {{ $t('settings.configHint') }}
      </p>
    </div>
  </UCard>
</template>
