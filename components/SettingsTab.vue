<script setup lang="ts">
import { loadDefaultsYaml } from '~/utils/defaultLayers'

const { config, configPath, flush } = useConfig()
const mapper = useMapper()

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

const deviceOptions = computed(() =>
  mapper.devices.value.map((d) => ({
    label: `${d.name}  —  ${d.path}`,
    value: d.path,
  })),
)

const selectedDevice = computed<string>({
  get: () => config.value.settings.inputDevicePath ?? '',
  set: (v: string) => {
    config.value.settings.inputDevicePath = v
  },
})

async function toggleMapper() {
  // Persist current config first so Rust reads the freshest rules from disk.
  await flush()
  if (mapper.status.value.running) {
    await mapper.stop()
  } else {
    if (!selectedDevice.value) return
    await mapper.start(selectedDevice.value)
  }
}

onMounted(async () => {
  await Promise.all([mapper.refreshDevices(), mapper.refreshStatus()])
})
</script>

<template>
  <div class="space-y-4">
    <UCard>
      <template #header>
        <div class="flex items-center justify-between gap-3">
          <h2 class="font-semibold">Key-mapper</h2>
          <UBadge
            :color="mapper.status.value.running ? 'success' : 'neutral'"
            variant="subtle"
          >
            {{ mapper.status.value.running ? 'активен' : 'остановлен' }}
          </UBadge>
        </div>
      </template>
      <div class="space-y-4">
        <UFormField
          label="Клавиатура"
          help="Выберите физическое устройство, события которого нужно перехватывать."
        >
          <div class="flex gap-2 items-center">
            <USelectMenu
              v-model="selectedDevice"
              :items="deviceOptions"
              value-key="value"
              placeholder="Не выбрано"
              class="flex-1"
              :disabled="mapper.status.value.running"
            />
            <UButton
              variant="ghost"
              icon="i-lucide-refresh-cw"
              aria-label="Обновить список"
              :disabled="mapper.status.value.running"
              @click="mapper.refreshDevices()"
            />
          </div>
        </UFormField>

        <div class="flex items-center gap-2">
          <UButton
            :color="mapper.status.value.running ? 'error' : 'primary'"
            :icon="
              mapper.status.value.running
                ? 'i-lucide-square'
                : 'i-lucide-play'
            "
            :loading="mapper.busy.value"
            :disabled="!mapper.status.value.running && !selectedDevice"
            @click="toggleMapper"
          >
            {{ mapper.status.value.running ? 'Остановить' : 'Запустить' }}
          </UButton>
          <span
            v-if="mapper.error.value"
            class="text-sm text-(--ui-error) break-all"
          >
            {{ mapper.error.value }}
          </span>
        </div>

        <p class="text-xs text-(--ui-text-muted)">
          Маппер читает события напрямую с <code>/dev/input/eventX</code>
          и эмитит через <code>uinput</code>. Нужен доступ к этим
          устройствам — см. README (группа <code>input</code> и udev-правило
          для <code>/dev/uinput</code>).
        </p>
      </div>
    </UCard>

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

        <div>
          <UFormField>
            <template #label>
              <FieldLabel
                label="Hold timeout по умолчанию, мс"
                hint="Определение одиночного нажатия vs удержания слоя. Если клавиша отпущена до истечения — срабатывает tap action, если удерживается дольше — активируется слой. Используется правилами, где не задано собственное значение."
              />
            </template>
            <UInput
              v-model.number="config.settings.defaultHoldTimeoutMs"
              type="number"
              min="0"
              class="w-40"
            />
          </UFormField>
        </div>

        <div class="grid grid-cols-1 md:grid-cols-2 gap-4 pt-2 border-t border-(--ui-border)">
          <UFormField>
            <template #label>
              <FieldLabel
                label="Пауза между шагами макроса, мс"
                hint="Глобальное значение по умолчанию. Используется, когда шаг макроса не задаёт собственное."
              />
            </template>
            <UInput
              v-model.number="config.settings.defaultMacroStepPauseMs"
              type="number"
              min="0"
              class="w-40"
            />
          </UFormField>
          <UFormField>
            <template #label>
              <FieldLabel
                label="Задержка модификатора, мс"
                hint="Глобальное значение: сколько ждать между нажатием модификатора (Shift/Ctrl/...) и основной клавиши внутри одного шага."
              />
            </template>
            <UInput
              v-model.number="config.settings.defaultMacroModifierDelayMs"
              type="number"
              min="0"
              class="w-40"
            />
          </UFormField>
        </div>
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
