<script setup lang="ts">
const { loaded, saving, lastError, load } = useConfig()

const tabItems = [
  { value: 'rules', slot: 'rules', label: 'Правила', icon: 'i-lucide-list-checks' },
  { value: 'keymap', slot: 'keymap', label: 'Keymap', icon: 'i-lucide-keyboard' },
  { value: 'settings', slot: 'settings', label: 'Настройки', icon: 'i-lucide-settings' },
] as const

const active = ref<string>('rules')

onMounted(() => {
  void load()
})
</script>

<template>
  <div class="min-h-screen flex flex-col">
    <header
      class="flex items-center justify-between px-6 py-3 border-b border-(--ui-border) bg-(--ui-bg-elevated)"
    >
      <div class="flex items-center gap-3">
        <h1 class="text-lg font-semibold">Left Hand Control</h1>
        <UBadge color="primary" variant="subtle">Linux key-mapper</UBadge>
      </div>
      <div class="text-xs text-(--ui-text-muted) flex items-center gap-3">
        <span v-if="!loaded">загрузка…</span>
        <span v-else-if="saving" class="flex items-center gap-1">
          <UIcon name="i-lucide-loader-2" class="animate-spin" />
          сохранение
        </span>
        <span v-else class="flex items-center gap-1 text-(--ui-success)">
          <UIcon name="i-lucide-check" />
          сохранено
        </span>
        <span v-if="lastError" class="text-(--ui-error)">
          {{ lastError }}
        </span>
      </div>
    </header>

    <main class="flex-1 p-6 w-full">
      <UTabs
        v-model="active"
        :items="tabItems"
        class="w-full"
      >
        <template #rules>
          <div class="mt-4">
            <RulesTab v-if="loaded" />
          </div>
        </template>
        <template #keymap>
          <div class="mt-4">
            <KeymapTab v-if="loaded" />
          </div>
        </template>
        <template #settings>
          <div class="mt-4">
            <SettingsTab v-if="loaded" />
          </div>
        </template>
      </UTabs>
    </main>
  </div>
</template>
