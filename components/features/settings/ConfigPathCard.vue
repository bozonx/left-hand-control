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
    <div class="grid gap-3 text-sm">
      <div class="grid gap-2 md:grid-cols-[13rem_minmax(0,1fr)] md:items-center">
        <div class="text-(--ui-text-muted)">{{ $t('settings.configPath') }}</div>
        <div class="flex min-w-0 items-center gap-2">
          <code class="min-w-0 flex-1 rounded bg-(--ui-bg-muted) px-2 py-1.5 break-all copyable">
            {{ configPath || '…' }}
          </code>
          <UButton
            size="xs"
            color="neutral"
            variant="outline"
            icon="i-lucide-copy"
            :disabled="!configPath"
            @click="copyPath(configPath)"
          />
        </div>
      </div>

      <div class="grid gap-2 md:grid-cols-[13rem_minmax(0,1fr)] md:items-center">
        <div class="text-(--ui-text-muted)">{{ $t('settings.layoutsPath') }}</div>
        <div class="flex min-w-0 items-center gap-2">
          <code class="min-w-0 flex-1 rounded bg-(--ui-bg-muted) px-2 py-1.5 break-all copyable">
            {{ layoutsDir || '…' }}
          </code>
          <UButton
            size="xs"
            color="neutral"
            variant="outline"
            icon="i-lucide-copy"
            :disabled="!layoutsDir"
            @click="copyPath(layoutsDir)"
          />
        </div>
      </div>
    </div>
  </UCard>
</template>
