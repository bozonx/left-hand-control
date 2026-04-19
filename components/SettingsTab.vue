<script setup lang="ts">
const { config, configPath } = useConfig()
</script>

<template>
  <div class="space-y-4">
    <UCard>
      <template #header>
        <h2 class="font-semibold">Общие</h2>
      </template>
      <div class="space-y-4">
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="font-medium">Запускать вместе с системой</div>
            <div class="text-xs text-(--ui-text-muted)">
              Пока не реализовано — переключатель сохраняется в конфиг, но не
              регистрирует autostart.
            </div>
          </div>
          <USwitch
            v-model="config.settings.launchOnStartup"
            disabled
          />
        </div>

        <UFormField
          label="Hold timeout по умолчанию, мс"
          help="Используется правилами, где не задано собственное значение."
        >
          <UInput
            v-model.number="config.settings.defaultHoldTimeoutMs"
            type="number"
            min="0"
            class="w-40"
          />
        </UFormField>
      </div>
    </UCard>

    <UCard>
      <template #header>
        <h2 class="font-semibold">Файл конфигурации</h2>
      </template>
      <div class="text-sm">
        <div class="text-(--ui-text-muted) mb-1">Путь:</div>
        <code class="block p-2 rounded bg-(--ui-bg-muted) break-all">
          {{ configPath || '…' }}
        </code>
        <p class="text-xs text-(--ui-text-muted) mt-2">
          Все изменения сохраняются автоматически.
        </p>
      </div>
    </UCard>
  </div>
</template>
