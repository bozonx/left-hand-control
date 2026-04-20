<script setup lang="ts">
import { BUILTIN_LAYOUT_ID } from '~/types/config'
import { emptyLayoutPreset, loadBuiltinLayout } from '~/utils/layoutPresets'

const { applyPreset } = useConfig()

const busy = ref<'' | 'ivank' | 'empty'>('')
const error = ref<string | null>(null)

async function pickIvanK() {
  busy.value = 'ivank'
  error.value = null
  try {
    const preset = await loadBuiltinLayout()
    if (!preset) {
      error.value = 'Не удалось загрузить встроенную раскладку.'
      return
    }
    await applyPreset(preset, BUILTIN_LAYOUT_ID)
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = ''
  }
}

async function pickEmpty() {
  busy.value = 'empty'
  error.value = null
  try {
    await applyPreset(emptyLayoutPreset(), undefined)
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = ''
  }
}
</script>

<template>
  <div class="min-h-screen flex items-center justify-center p-6 bg-(--ui-bg)">
    <div class="max-w-3xl w-full space-y-6">
      <div class="text-center space-y-2">
        <h1 class="text-3xl font-bold">Left Hand Control</h1>
        <p class="text-(--ui-text-muted)">
          Добро пожаловать! Выберите стартовую раскладку — её можно будет
          поменять в любой момент в настройках.
        </p>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <UCard class="flex flex-col">
          <template #header>
            <div class="flex items-center gap-2">
              <UIcon name="i-lucide-sparkles" class="text-(--ui-primary)" />
              <h2 class="font-semibold">Ivan K's left hand control</h2>
              <UBadge color="primary" variant="subtle" size="sm">
                рекомендуется
              </UBadge>
            </div>
          </template>
          <p class="text-sm text-(--ui-text-muted) flex-1">
            Готовая авторская раскладка: CapsLock — слой навигации,
            правый Alt — слой символов, левый Alt — оконный менеджер.
            Включает набор макросов для IDE.
          </p>
          <template #footer>
            <UButton
              block
              color="primary"
              icon="i-lucide-check"
              :loading="busy === 'ivank'"
              :disabled="!!busy"
              @click="pickIvanK"
            >
              Использовать эту раскладку
            </UButton>
          </template>
        </UCard>

        <UCard class="flex flex-col">
          <template #header>
            <div class="flex items-center gap-2">
              <UIcon name="i-lucide-file-plus" />
              <h2 class="font-semibold">Пустая раскладка</h2>
            </div>
          </template>
          <p class="text-sm text-(--ui-text-muted) flex-1">
            Начать с чистого листа: только базовый слой, без правил и
            макросов. Подходит, если вы хотите собрать раскладку с нуля.
          </p>
          <template #footer>
            <UButton
              block
              color="neutral"
              variant="outline"
              icon="i-lucide-plus"
              :loading="busy === 'empty'"
              :disabled="!!busy"
              @click="pickEmpty"
            >
              Начать с пустой
            </UButton>
          </template>
        </UCard>
      </div>

      <p v-if="error" class="text-sm text-(--ui-error) text-center">
        {{ error }}
      </p>

      <p class="text-xs text-(--ui-text-muted) text-center">
        Выбранную раскладку всегда можно заменить в разделе
        <b>Настройки → Раскладки</b>.
      </p>
    </div>
  </div>
</template>
