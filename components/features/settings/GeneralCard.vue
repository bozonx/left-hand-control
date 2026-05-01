<script setup lang="ts">
import type {
  AppConfig,
  AppearancePreference,
  LocalePreference,
} from '~/types/config'

const props = defineProps<{
  config: AppConfig
  resolvedTheme: 'light' | 'dark'
  appearanceItems: Array<{ label: string, value: AppearancePreference }>
  localeItems: Array<{ label: string, value: LocalePreference }>
}>()

const themePreference = defineModel<AppearancePreference>('themePreference', { required: true })
const localePreference = defineModel<LocalePreference>('localePreference', { required: true })
</script>

<template>
  <UCard>
    <template #header>
      <h2 class="text-sm font-semibold">{{ $t('settings.generalTitle') }}</h2>
    </template>
    <div class="space-y-4">
      <div class="flex items-center justify-between gap-4">
        <div>
          <div class="text-xs font-medium text-(--ui-text-muted)">{{ $t('settings.appearance') }}</div>
          <div class="text-xs text-(--ui-text-muted)">
            <i18n-t keypath="settings.appearanceHint" tag="span">
              <template #pref><code>prefers-color-scheme</code></template>
              <template #mode>
                <span class="font-medium">
                  {{
                    props.resolvedTheme === 'dark'
                      ? $t('settings.appearanceDark')
                      : $t('settings.appearanceLight')
                  }}
                </span>
              </template>
            </i18n-t>
          </div>
        </div>
        <URadioGroup
          v-model="themePreference"
          :items="appearanceItems"
          orientation="horizontal"
          size="sm"
        />
      </div>

      <div class="flex items-center justify-between gap-4 pt-2 border-t border-(--ui-border)">
        <div>
          <div class="text-xs font-medium text-(--ui-text-muted)">{{ $t('settings.language') }}</div>
          <div class="text-xs text-(--ui-text-muted)">
            {{ $t('settings.languageHint') }}
          </div>
        </div>
        <USelectMenu
          v-model="localePreference"
          :items="localeItems"
          value-key="value"
          class="min-w-[220px]"
        />
      </div>

    </div>
  </UCard>
</template>
