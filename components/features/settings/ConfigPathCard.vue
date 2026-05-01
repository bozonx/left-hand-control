<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'

const props = defineProps<{
  settingsDir: string
  layoutsDir: string
}>()

const { copy } = useClipboardCopy()

async function copyPath(path: string) {
  if (!path) return
  await copy(path)
}
</script>

<template>
  <UCard>
    <template #header>
      <h2 class="text-sm font-semibold">{{ $t('settings.configTitle') }}</h2>
    </template>
    <div class="grid gap-3 text-sm">
      <div class="grid gap-2 md:grid-cols-[13rem_minmax(0,1fr)] md:items-center">
        <div class="text-(--ui-text-muted)">{{ $t('settings.settingsDirPath') }}</div>
        <div class="flex min-w-0 items-center gap-2">
          <code class="min-w-0 flex-1 rounded bg-(--ui-bg-muted) px-2 py-1.5 break-all copyable">
            {{ settingsDir || '…' }}
          </code>
          <AppTooltip :text="$t('settings.copyPathTooltip')">
            <UButton
              size="xs"
              color="neutral"
              variant="outline"
              icon="i-lucide-copy"
              :disabled="!settingsDir"
              @click="copyPath(settingsDir)"
            />
          </AppTooltip>
        </div>
      </div>

      <div class="grid gap-2 md:grid-cols-[13rem_minmax(0,1fr)] md:items-center">
        <div class="text-(--ui-text-muted)">{{ $t('settings.layoutsPath') }}</div>
        <div class="flex min-w-0 items-center gap-2">
          <code class="min-w-0 flex-1 rounded bg-(--ui-bg-muted) px-2 py-1.5 break-all copyable">
            {{ layoutsDir || '…' }}
          </code>
          <AppTooltip :text="$t('settings.copyPathTooltip')">
            <UButton
              size="xs"
              color="neutral"
              variant="outline"
              icon="i-lucide-copy"
              :disabled="!layoutsDir"
              @click="copyPath(layoutsDir)"
            />
          </AppTooltip>
        </div>
      </div>
    </div>
  </UCard>
</template>
