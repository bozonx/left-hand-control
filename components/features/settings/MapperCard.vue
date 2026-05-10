<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import FieldLabel from '~/components/FieldLabel.vue'
import type { MapperState } from '~/composables/useMapper'

interface SettingsIssue {
  id: string
  severity: 'error' | 'warning'
  title: string
  description: string
}

interface DeviceOption {
  label: string
  value: string
}

interface DeviceOptionLabel {
  type: 'label'
  label: string
}

type DeviceOptionGroup = Array<DeviceOption | DeviceOptionLabel>

const MANUAL_DEVICE_VALUE = '__manual_device_path__'

const props = defineProps<{
  mapper: MapperState
  deviceOptions: DeviceOptionGroup[]
  selectedDevice: string
  mouseOptions: DeviceOptionGroup[]
  selectedMouse: string
  issues: SettingsIssue[]
}>()

const emit = defineEmits<{
  'update:selectedDevice': [value: string]
  'update:selectedMouse': [value: string]
  toggle: []
}>()

const hasErrorIssues = computed(() =>
  props.issues.some((issue) => issue.severity === 'error'),
)

const manualKeyboard = ref(false)
const manualMouse = ref(false)

function optionValues(groups: DeviceOptionGroup[]) {
  return groups.flatMap((group) =>
    group.flatMap((item) => 'value' in item ? [item.value] : []),
  )
}

const keyboardSelectValue = computed(() => {
  if (manualKeyboard.value) return MANUAL_DEVICE_VALUE
  if (!props.selectedDevice) return ''
  return optionValues(props.deviceOptions).includes(props.selectedDevice)
    ? props.selectedDevice
    : MANUAL_DEVICE_VALUE
})

const mouseSelectValue = computed(() => {
  if (manualMouse.value) return MANUAL_DEVICE_VALUE
  if (!props.selectedMouse) return ''
  return optionValues(props.mouseOptions).includes(props.selectedMouse)
    ? props.selectedMouse
    : MANUAL_DEVICE_VALUE
})

const showManualKeyboard = computed(() => keyboardSelectValue.value === MANUAL_DEVICE_VALUE)
const showManualMouse = computed(() => mouseSelectValue.value === MANUAL_DEVICE_VALUE)

function selectKeyboard(value: string) {
  manualKeyboard.value = value === MANUAL_DEVICE_VALUE
  if (manualKeyboard.value) {
    emit('update:selectedDevice', '')
    return
  }
  emit('update:selectedDevice', value)
}

function selectMouse(value: string) {
  manualMouse.value = value === MANUAL_DEVICE_VALUE
  if (manualMouse.value) {
    emit('update:selectedMouse', '')
    return
  }
  emit('update:selectedMouse', value)
}
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between gap-3">
        <h2 class="text-sm font-semibold">{{ $t('settings.mapperTitle') }}</h2>
        <div class="flex items-center gap-2">
          <AppTooltip :text="$t('settings.refreshDevicesTooltip')">
            <UButton
              variant="ghost"
              icon="i-lucide-refresh-cw"
              :aria-label="$t('settings.refreshDevices')"
              :disabled="mapper.status.value.running"
              @click="mapper.refreshDevices()"
            />
          </AppTooltip>
          <UBadge
            :color="mapper.status.value.running ? 'success' : 'neutral'"
            variant="subtle"
          >
            {{ mapper.status.value.running ? $t('common.active') : $t('common.stopped') }}
          </UBadge>
        </div>
      </div>
    </template>
    <div class="space-y-4">
      <UAlert
        v-for="issue in issues"
        :key="issue.id"
        :color="issue.severity"
        variant="soft"
        :icon="issue.severity === 'error' ? 'i-lucide-circle-alert' : 'i-lucide-triangle-alert'"
        :title="issue.title"
        :description="issue.description"
      />

      <UFormField>
        <template #label>
          <FieldLabel
            :label="$t('settings.keyboardLabel')"
            :hint="$t('settings.keyboardHelp')"
          />
        </template>
        <USelectMenu
          id="mapper-keyboard-device"
          :model-value="keyboardSelectValue"
          :items="deviceOptions"
          value-key="value"
          :placeholder="$t('settings.devicePh')"
          class="w-full"
          :disabled="mapper.status.value.running"
          @update:model-value="selectKeyboard"
        />
        <UInput
          v-if="showManualKeyboard"
          :model-value="selectedDevice"
          :placeholder="$t('settings.manualDevicePlaceholder')"
          class="mt-2 w-full"
          :disabled="mapper.status.value.running"
          @update:model-value="(value: string) => $emit('update:selectedDevice', value)"
        />
      </UFormField>

      <UFormField>
        <template #label>
          <FieldLabel
            :label="$t('settings.mouseLabel')"
            :hint="$t('settings.mouseHelp')"
          />
        </template>
        <USelectMenu
          id="mapper-mouse-device"
          :model-value="mouseSelectValue"
          :items="mouseOptions"
          value-key="value"
          :placeholder="$t('settings.mouseDevicePh')"
          class="w-full"
          :disabled="mapper.status.value.running"
          @update:model-value="selectMouse"
        />
        <UInput
          v-if="showManualMouse"
          :model-value="selectedMouse"
          :placeholder="$t('settings.manualDevicePlaceholder')"
          class="mt-2 w-full"
          :disabled="mapper.status.value.running"
          @update:model-value="(value: string) => $emit('update:selectedMouse', value)"
        />
        <p class="mt-2 text-xs text-(--ui-text-muted)">
          {{ $t('settings.mouseNativeActionNote') }}
        </p>
      </UFormField>

      <div class="flex items-center gap-2">
        <UButton
          :color="mapper.status.value.running ? 'error' : 'primary'"
          :variant="mapper.status.value.running ? 'outline' : 'solid'"
          :icon="
            mapper.status.value.running
              ? 'i-lucide-square'
              : 'i-lucide-play'
          "
          :loading="mapper.busy.value"
          :disabled="!mapper.status.value.running && (!selectedDevice || hasErrorIssues)"
          @click="$emit('toggle')"
        >
          {{ mapper.status.value.running ? $t('settings.stop') : $t('settings.start') }}
        </UButton>
        <span
          v-if="mapper.error.value"
          class="text-sm text-(--ui-error) break-all"
        >
          {{ mapper.error.value }}
        </span>
      </div>

      <p class="text-xs text-(--ui-text-muted)">
        <i18n-t keypath="settings.mapperHint" tag="span">
          <template #input><code>/dev/input/eventX</code></template>
          <template #uinput><code>uinput</code></template>
          <template #group><code>input</code></template>
          <template #uinputDev><code>/dev/uinput</code></template>
        </i18n-t>
      </p>
    </div>
  </UCard>
</template>
