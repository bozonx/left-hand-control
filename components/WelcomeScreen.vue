<script setup lang="ts">
import { BUILTIN_LAYOUT_ID } from '~/types/config'
import { emptyLayoutPreset, loadBuiltinLayout } from '~/utils/layoutPresets'

const { applyPreset } = useConfig()
const { t } = useI18n()

const busy = ref<'' | 'ivank' | 'empty'>('')
const error = ref<string | null>(null)

async function pickIvanK() {
  busy.value = 'ivank'
  error.value = null
  try {
    const preset = await loadBuiltinLayout()
    if (!preset) {
      error.value = t('welcome.loadError')
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
    await applyPreset(emptyLayoutPreset(t('settings.emptyLayoutName')), undefined)
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
        <h1 class="text-3xl font-bold">{{ $t('app.title') }}</h1>
        <p class="text-(--ui-text-muted)">
          {{ $t('welcome.intro') }}
        </p>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <UCard class="flex flex-col">
          <template #header>
            <div class="flex items-center gap-2">
              <UIcon name="i-lucide-sparkles" class="text-(--ui-primary)" />
              <h2 class="font-semibold">{{ $t('welcome.ivankTitle') }}</h2>
              <UBadge color="primary" variant="subtle" size="sm">
                {{ $t('welcome.recommended') }}
              </UBadge>
            </div>
          </template>
          <p class="text-sm text-(--ui-text-muted) flex-1">
            {{ $t('welcome.ivankDesc') }}
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
              {{ $t('welcome.useThis') }}
            </UButton>
          </template>
        </UCard>

        <UCard class="flex flex-col">
          <template #header>
            <div class="flex items-center gap-2">
              <UIcon name="i-lucide-file-plus" />
              <h2 class="font-semibold">{{ $t('welcome.emptyTitle') }}</h2>
            </div>
          </template>
          <p class="text-sm text-(--ui-text-muted) flex-1">
            {{ $t('welcome.emptyDesc') }}
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
              {{ $t('welcome.emptyBtn') }}
            </UButton>
          </template>
        </UCard>
      </div>

      <p v-if="error" class="text-sm text-(--ui-error) text-center">
        {{ error }}
      </p>

      <p class="text-xs text-(--ui-text-muted) text-center">
        <i18n-t keypath="welcome.footnote" tag="span">
          <template #path>
            <b>{{ $t('welcome.footnotePath') }}</b>
          </template>
        </i18n-t>
      </p>
    </div>
  </div>
</template>
