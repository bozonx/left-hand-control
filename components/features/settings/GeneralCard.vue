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

// Placeholder only: the flag is persisted in config, but real OS autostart
// registration is not implemented yet.

function setNonNegativeInt(
  key:
    | 'defaultHoldTimeoutMs'
    | 'defaultDoubleTapTimeoutMs'
    | 'defaultMacroStepPauseMs'
    | 'defaultMacroModifierDelayMs',
  value: string | number,
) {
  const parsed = typeof value === 'number' ? value : Number(value)
  if (!Number.isFinite(parsed)) return
  props.config.settings[key] = Math.max(0, Math.round(parsed))
}
</script>

<template>
  <UCard>
    <template #header>
      <h2 class="font-semibold">{{ $t('settings.generalTitle') }}</h2>
    </template>
    <div class="space-y-4">
      <div class="flex items-center justify-between gap-4">
        <div>
          <div class="font-medium">{{ $t('settings.appearance') }}</div>
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
          <div class="font-medium">{{ $t('settings.language') }}</div>
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

      <div class="flex items-center justify-between gap-4 pt-2 border-t border-(--ui-border)">
        <div>
          <div class="font-medium flex items-center gap-2">
            {{ $t('settings.launchOnStartup') }}
            <UBadge
              color="neutral"
              variant="outline"
              size="sm"
            >
              {{ $t('settings.stubBadge') }}
            </UBadge>
          </div>
          <div class="text-xs text-(--ui-text-muted)">
            {{ $t('settings.launchOnStartupHint') }}
          </div>
        </div>
        <USwitch
          v-model="props.config.settings.launchOnStartup"
          disabled
        />
      </div>

      <div class="grid grid-cols-2 gap-4">
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.holdTimeout')"
              :hint="$t('settings.holdTimeoutHint')"
            />
          </template>
          <UInput
            :model-value="String(props.config.settings.defaultHoldTimeoutMs)"
            type="number"
            min="0"
            class="w-40"
            @update:model-value="(value) => setNonNegativeInt('defaultHoldTimeoutMs', value)"
          />
        </UFormField>
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.doubleTapTimeout')"
              :hint="$t('settings.doubleTapTimeoutHint')"
            />
          </template>
          <UInput
            :model-value="String(props.config.settings.defaultDoubleTapTimeoutMs)"
            type="number"
            min="0"
            class="w-40"
            @update:model-value="(value) => setNonNegativeInt('defaultDoubleTapTimeoutMs', value)"
          />
        </UFormField>
      </div>

      <div class="grid grid-cols-2 gap-4 pt-2 border-t border-(--ui-border)">
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.stepPauseLabel')"
              :hint="$t('settings.stepPauseHint')"
            />
          </template>
          <UInput
            :model-value="String(props.config.settings.defaultMacroStepPauseMs)"
            type="number"
            min="0"
            class="w-40"
            @update:model-value="(value) => setNonNegativeInt('defaultMacroStepPauseMs', value)"
          />
        </UFormField>
        <UFormField>
          <template #label>
            <FieldLabel
              :label="$t('settings.modDelayLabel')"
              :hint="$t('settings.modDelayHint')"
            />
          </template>
          <UInput
            :model-value="String(props.config.settings.defaultMacroModifierDelayMs)"
            type="number"
            min="0"
            class="w-40"
            @update:model-value="(value) => setNonNegativeInt('defaultMacroModifierDelayMs', value)"
          />
        </UFormField>
      </div>
    </div>
  </UCard>
</template>
