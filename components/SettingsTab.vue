<script setup lang="ts">
import { loadDefaultsYaml } from '~/utils/defaultLayers'

const { config, configPath, flush } = useConfig()

const resetting = ref(false)
const resetConfirmOpen = ref(false)

async function resetToDefaults() {
  resetting.value = true
  try {
    const defaults = await loadDefaultsYaml()
    if (defaults) {
      config.value = defaults
      await flush()
    }
  } finally {
    resetting.value = false
    resetConfirmOpen.value = false
  }
}
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

    <UCard>
      <template #header>
        <h2 class="font-semibold">Дефолтные слои</h2>
      </template>
      <div class="space-y-3">
        <p class="text-sm text-(--ui-text-muted)">
          Перезаписать текущий конфиг шаблоном из
          <code>public/default-layers.yaml</code>. Текущие правила, слои и
          раскладки будут потеряны.
        </p>
        <UButton
          color="warning"
          variant="outline"
          icon="i-lucide-rotate-ccw"
          :loading="resetting"
          @click="resetConfirmOpen = true"
        >
          Сбросить к дефолтам
        </UButton>
      </div>
    </UCard>

    <UModal v-model:open="resetConfirmOpen" title="Сбросить к дефолтам?">
      <template #body>
        <p class="text-sm">
          Текущие настройки будут заменены содержимым
          <code>default-layers.yaml</code> и сохранены в
          <code>config.json</code>. Действие необратимо.
        </p>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton
            variant="ghost"
            color="neutral"
            @click="resetConfirmOpen = false"
          >
            Отмена
          </UButton>
          <UButton
            color="warning"
            :loading="resetting"
            @click="resetToDefaults"
          >
            Сбросить
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
