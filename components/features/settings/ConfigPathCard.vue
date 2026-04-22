<script setup lang="ts">
const props = defineProps<{
  configPath: string
  layoutsDir: string
}>()

const toast = useToast()
const { t } = useI18n()

async function copyPath(path: string) {
  if (!path) return
  try {
    await navigator.clipboard.writeText(path)
    toast.add({
      title: t('common.copied'),
      description: path,
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
      <h2 class="text-sm font-semibold">{{ $t('settings.configTitle') }}</h2>
    </template>
    <div class="text-sm space-y-4">
      <div>
        <div class="mb-2 flex items-center justify-between gap-3">
          <div class="text-(--ui-text-muted)">{{ $t('settings.configPath') }}</div>
          <UButton
            size="xs"
            color="neutral"
            variant="outline"
            icon="i-lucide-copy"
            :disabled="!configPath"
            @click="copyPath(configPath)"
          >
            {{ $t('common.copy') }}
          </UButton>
        </div>
        <code class="block p-2 rounded bg-(--ui-bg-muted) break-all copyable">
          {{ configPath || '…' }}
        </code>
      </div>
      <div>
        <div class="mb-2 flex items-center justify-between gap-3">
          <div class="text-(--ui-text-muted)">{{ $t('settings.layoutsPath') }}</div>
          <UButton
            size="xs"
            color="neutral"
            variant="outline"
            icon="i-lucide-copy"
            :disabled="!layoutsDir"
            @click="copyPath(layoutsDir)"
          >
            {{ $t('common.copy') }}
          </UButton>
        </div>
        <code class="block p-2 rounded bg-(--ui-bg-muted) break-all copyable">
          {{ layoutsDir || '…' }}
        </code>
      </div>
      <p class="text-xs text-(--ui-text-muted) mt-2">
        {{ $t('settings.configHint') }}
      </p>
    </div>
  </UCard>
</template>
