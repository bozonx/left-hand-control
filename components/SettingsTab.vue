<script setup lang="ts">
import MapperCard from '~/components/features/settings/MapperCard.vue'
import GeneralCard from '~/components/features/settings/GeneralCard.vue'
import LayoutsLibraryCard from '~/components/features/settings/LayoutsLibraryCard.vue'
import ConfigPathCard from '~/components/features/settings/ConfigPathCard.vue'
import { BUILTIN_LAYOUT_ID } from '~/types/config'
import {
  type LayoutLibraryEntry,
  isUserLayoutId,
  userLayoutId,
  userLayoutNameFromId,
} from '~/composables/useLayoutLibrary'
import {
  emptyLayoutPreset,
  extractPresetFromConfig,
  loadBuiltinLayout,
} from '~/utils/layoutPresets'
import { localeDisplayName } from '~/i18n'

const {
  config,
  configPath,
  flush,
  applyPreset,
  markLayoutSavedAs,
  currentLayoutId,
  isLayoutDirty,
} = useConfig()
const library = useLayoutLibrary()
const mapper = useMapper()
const theme = useAppTheme()
const appLocale = useAppLocale()
const { t } = useI18n()

const appearanceItems = computed(() => [
  { label: t('settings.appearanceItems.system'), value: 'system' },
  { label: t('settings.appearanceItems.light'), value: 'light' },
  { label: t('settings.appearanceItems.dark'), value: 'dark' },
])

// Locale picker options. The first row is the "Auto" (follow-OS) choice;
// the remaining rows list each supported locale by its own native name.
const localeItems = computed(() => {
  const resolvedName = localeDisplayName(appLocale.systemLocale.value)
  return [
    {
      label: t('settings.languageAutoResolved', { resolved: resolvedName }),
      value: 'auto' as const,
    },
    ...appLocale.available.map((loc) => ({
      label: localeDisplayName(loc),
      value: loc,
    })),
  ]
})

// --- Layout library ---------------------------------------------------------

const applying = ref<string>('') // entry id being applied
const applyError = ref<string | null>(null)

// Confirmation state for switching layouts. `pendingApply` holds the target
// we'll switch to after the user confirms.
interface PendingApply {
  kind: 'entry' | 'empty'
  entry?: LayoutLibraryEntry
  label: string
}
const pendingApply = ref<PendingApply | null>(null)

function requestApply(target: PendingApply) {
  applyError.value = null
  pendingApply.value = target
}

async function confirmApply() {
  const target = pendingApply.value
  if (!target) return
  const id = target.kind === 'empty' ? 'empty' : target.entry!.id
  applying.value = id
  try {
    if (target.kind === 'empty') {
      await applyPreset(emptyLayoutPreset(t('settings.emptyLayoutName')), undefined)
    } else {
      const entry = target.entry!
      const preset =
        entry.id === BUILTIN_LAYOUT_ID
          ? await loadBuiltinLayout()
          : await library.loadPreset(entry.id)
      if (!preset) {
        applyError.value = t('settings.loadFailed', { name: entry.name })
        return
      }
      await applyPreset(preset, entry.id)
    }
    pendingApply.value = null
  } catch (e) {
    applyError.value = e instanceof Error ? e.message : String(e)
  } finally {
    applying.value = ''
  }
}

function cancelApply() {
  pendingApply.value = null
}

// --- Save current as user layout -------------------------------------------

const saveModalOpen = ref(false)
const saveName = ref('')
const saveBusy = ref(false)
const saveError = ref<string | null>(null)

function openSaveModal() {
  saveError.value = null
  // Pre-fill with the current layout name if it's a user one.
  if (isUserLayoutId(currentLayoutId.value)) {
    saveName.value = userLayoutNameFromId(currentLayoutId.value!)
  } else {
    saveName.value = ''
  }
  saveModalOpen.value = true
}

async function performSave() {
  const name = saveName.value.trim()
  if (!name) {
    saveError.value = t('settings.saveErrorEmpty')
    return
  }
  saveBusy.value = true
  saveError.value = null
  try {
    const preset = extractPresetFromConfig(config.value, name)
    const savedName = await library.saveUserPreset(name, preset)
    await markLayoutSavedAs(userLayoutId(savedName))
    saveModalOpen.value = false
  } catch (e) {
    saveError.value = e instanceof Error ? e.message : String(e)
  } finally {
    saveBusy.value = false
  }
}

// --- Delete user layout -----------------------------------------------------

const deletePending = ref<LayoutLibraryEntry | null>(null)
const deleteBusy = ref(false)

async function confirmDelete() {
  const entry = deletePending.value
  if (!entry || entry.builtin) return
  deleteBusy.value = true
  try {
    await library.deleteUserPreset(userLayoutNameFromId(entry.id))
    // If we just deleted the active layout, clear the link (but keep content).
    if (currentLayoutId.value === entry.id) {
      config.value.settings.currentLayoutId = undefined
      await flush()
    }
    deletePending.value = null
  } finally {
    deleteBusy.value = false
  }
}

onMounted(async () => {
  await Promise.all([
    mapper.refreshDevices(),
    mapper.refreshStatus(),
    library.refresh(),
  ])
})

// --- Mapper / devices -------------------------------------------------------

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
</script>

<template>
  <div class="space-y-4">
    <MapperCard
      v-model:selected-device="selectedDevice"
      :mapper="mapper"
      :device-options="deviceOptions"
      @toggle="toggleMapper"
    />

    <GeneralCard
      :config="config"
      v-model:theme-preference="theme.preference.value"
      :resolved-theme="theme.resolved.value"
      v-model:locale-preference="appLocale.preference.value"
      :appearance-items="appearanceItems"
      :locale-items="localeItems"
    />

    <LayoutsLibraryCard
      :entries="library.entries.value"
      :current-layout-id="currentLayoutId"
      :is-layout-dirty="isLayoutDirty"
      :applying="applying"
      :apply-error="applyError"
      :layouts-dir="library.layoutsDir.value"
      @save-current="openSaveModal"
      @request-apply-entry="(entry) => requestApply({ kind: 'entry', entry, label: entry.name })"
      @request-apply-empty="requestApply({ kind: 'empty', label: $t('settings.emptyLayoutName') })"
      @request-delete="(entry) => deletePending = entry"
    />

    <ConfigPathCard :config-path="configPath" />

    <!-- Apply confirmation -->
    <UModal
      :open="!!pendingApply"
      :title="$t('settings.confirmApplyTitle', { label: pendingApply?.label ?? '' })"
      @update:open="(v) => !v && cancelApply()"
    >
      <template #body>
        <div class="space-y-3 text-sm">
          <p>
            {{ $t('settings.confirmApplyBody') }}
          </p>
          <div
            v-if="isLayoutDirty"
            class="flex items-start gap-2 p-3 rounded border border-(--ui-error)/40 bg-(--ui-error)/10"
          >
            <UIcon
              name="i-lucide-alert-triangle"
              class="text-(--ui-error) mt-0.5 shrink-0"
            />
            <div>
              <div class="font-semibold">
                {{ $t('settings.dirtyWarnTitle') }}
              </div>
              <div>
                <i18n-t keypath="settings.dirtyWarnBody" tag="span">
                  <template #btn>
                    <b>«{{ $t('settings.saveCurrent') }}»</b>
                  </template>
                </i18n-t>
              </div>
            </div>
          </div>
        </div>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton variant="ghost" color="neutral" @click="cancelApply">
            {{ $t('common.cancel') }}
          </UButton>
          <UButton
            :color="isLayoutDirty ? 'error' : 'primary'"
            :loading="!!applying"
            @click="confirmApply"
          >
            {{ isLayoutDirty ? $t('settings.loseAndSwitch') : $t('settings.switch') }}
          </UButton>
        </div>
      </template>
    </UModal>

    <!-- Save-as modal -->
    <UModal
      v-model:open="saveModalOpen"
      :title="$t('settings.saveModalTitle')"
    >
      <template #body>
        <div class="space-y-3">
          <UFormField :label="$t('settings.nameLabel')">
            <UInput
              v-model="saveName"
              :placeholder="$t('settings.namePh')"
              autofocus
              @keyup.enter="performSave"
            />
          </UFormField>
          <p class="text-xs text-(--ui-text-muted)">
            <i18n-t keypath="settings.saveHint" tag="span">
              <template #path>
                <code class="break-all">{{ library.layoutsDir.value }}/{{ saveName || '…' }}.yaml</code>
              </template>
            </i18n-t>
          </p>
          <p v-if="saveError" class="text-sm text-(--ui-error)">
            {{ saveError }}
          </p>
        </div>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton
            variant="ghost"
            color="neutral"
            @click="saveModalOpen = false"
          >
            {{ $t('common.cancel') }}
          </UButton>
          <UButton
            color="primary"
            icon="i-lucide-save"
            :loading="saveBusy"
            :disabled="!saveName.trim()"
            @click="performSave"
          >
            {{ $t('common.save') }}
          </UButton>
        </div>
      </template>
    </UModal>

    <!-- Delete confirmation -->
    <UModal
      :open="!!deletePending"
      :title="$t('settings.deleteTitle', { name: deletePending?.name ?? '' })"
      @update:open="(v) => !v && (deletePending = null)"
    >
      <template #body>
        <p class="text-sm">
          {{ $t('settings.deleteBody') }}
        </p>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton
            variant="ghost"
            color="neutral"
            @click="deletePending = null"
          >
            {{ $t('common.cancel') }}
          </UButton>
          <UButton
            color="error"
            :loading="deleteBusy"
            @click="confirmDelete"
          >
            {{ $t('common.delete') }}
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
