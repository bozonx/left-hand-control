<script setup lang="ts">
const props = defineProps<{
  configPath: string
}>()

const toast = useToast()
const { t } = useI18n()

async function copyPath() {
  if (!props.configPath) return
  try {
    await navigator.clipboard.writeText(props.configPath)
    toast.add({
      title: t('common.copied'),
      description: props.configPath,
      icon: 'i-lucide-copy-check',
      close: true,
    })
  } catch (error) {
    toast.add({
      title: t('common.copy'),
      description: error instanceof Error ? error.message : String(error),
      color: 'error',
      icon: 'i-lucide-circle-alert',
      close: true,
    })
  }
}
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <h2 class="font-semibold">{{ $t('settings.configTitle') }}</h2>
        <UButton
          size="sm"
          color="neutral"
          variant="outline"
          icon="i-lucide-copy"
          :disabled="!configPath"
          @click="copyPath"
        >
          {{ $t('common.copy') }}
        </UButton>
      </div>
    </template>
    <div class="text-sm">
      <div class="text-(--ui-text-muted) mb-1">{{ $t('settings.configPath') }}</div>
      <code class="block p-2 rounded bg-(--ui-bg-muted) break-all copyable">
        {{ configPath || '…' }}
      </code>
      <p class="text-xs text-(--ui-text-muted) mt-2">
        {{ $t('settings.configHint') }}
      </p>
    </div>
  </UCard>
</template>
