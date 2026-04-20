<script setup lang="ts">
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
      await applyPreset(emptyLayoutPreset(), undefined)
    } else {
      const entry = target.entry!
      const preset =
        entry.id === BUILTIN_LAYOUT_ID
          ? await loadBuiltinLayout()
          : await library.loadPreset(entry.id)
      if (!preset) {
        applyError.value = `Не удалось загрузить раскладку "${entry.name}"`
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
    saveError.value = 'Введите имя раскладки.'
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
        <div class="flex items-center justify-between gap-3 flex-wrap">
          <h2 class="font-semibold">Раскладки</h2>
          <UButton
            color="primary"
            icon="i-lucide-save"
            :disabled="!isLayoutDirty && !currentLayoutId"
            @click="openSaveModal"
          >
            Сохранить текущую…
          </UButton>
        </div>
      </template>

      <div class="space-y-3">
        <div
          v-if="isLayoutDirty"
          class="flex items-start gap-2 p-3 rounded border border-(--ui-warning)/40 bg-(--ui-warning)/10 text-sm"
        >
          <UIcon
            name="i-lucide-alert-triangle"
            class="text-(--ui-warning) mt-0.5 shrink-0"
          />
          <div>
            <div class="font-semibold">
              В текущей раскладке есть несохранённые изменения.
            </div>
            <div class="text-(--ui-text-muted)">
              Переключение на другую раскладку затрёт правила, слои,
              раскладки и макросы. Сохраните текущую как пользовательскую,
              чтобы не потерять.
            </div>
          </div>
        </div>

        <p v-if="applyError" class="text-sm text-(--ui-error)">
          {{ applyError }}
        </p>

        <ul class="divide-y divide-(--ui-border) border border-(--ui-border) rounded">
          <li
            v-for="entry in library.entries.value"
            :key="entry.id"
            class="flex items-center justify-between gap-3 p-3"
          >
            <div class="flex items-center gap-2 min-w-0">
              <UIcon
                :name="entry.builtin ? 'i-lucide-sparkles' : 'i-lucide-file'"
                :class="entry.builtin ? 'text-(--ui-primary)' : ''"
              />
              <div class="min-w-0">
                <div class="font-medium truncate flex items-center gap-2">
                  {{ entry.name }}
                  <UBadge
                    v-if="currentLayoutId === entry.id"
                    color="success"
                    variant="subtle"
                    size="sm"
                  >
                    активна
                  </UBadge>
                  <UBadge
                    v-if="entry.builtin"
                    color="primary"
                    variant="outline"
                    size="sm"
                  >
                    встроенная
                  </UBadge>
                </div>
              </div>
            </div>
            <div class="flex items-center gap-2 shrink-0">
              <UButton
                variant="outline"
                icon="i-lucide-rotate-ccw"
                :loading="applying === entry.id"
                :disabled="!!applying"
                @click="
                  requestApply({
                    kind: 'entry',
                    entry,
                    label: entry.name,
                  })
                "
              >
                Применить
              </UButton>
              <UButton
                v-if="!entry.builtin"
                color="error"
                variant="ghost"
                icon="i-lucide-trash-2"
                aria-label="Удалить"
                @click="deletePending = entry"
              />
            </div>
          </li>
        </ul>

        <div class="flex items-center justify-between gap-3 pt-2 border-t border-(--ui-border)">
          <div class="text-sm text-(--ui-text-muted)">
            Сбросить всё: обнулить слои, правила, раскладки и макросы.
            Настройки приложения сохраняются.
          </div>
          <UButton
            color="warning"
            variant="outline"
            icon="i-lucide-eraser"
            :loading="applying === 'empty'"
            :disabled="!!applying"
            @click="
              requestApply({
                kind: 'empty',
                label: 'Пустая раскладка',
              })
            "
          >
            Сбросить всё
          </UButton>
        </div>

        <p class="text-xs text-(--ui-text-muted)">
          Папка с пользовательскими раскладками:
          <code class="break-all">{{ library.layoutsDir.value || '…' }}</code>
        </p>
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

    <!-- Apply confirmation -->
    <UModal
      :open="!!pendingApply"
      :title="`Переключиться на «${pendingApply?.label ?? ''}»?`"
      @update:open="(v) => !v && cancelApply()"
    >
      <template #body>
        <div class="space-y-3 text-sm">
          <p>
            Текущие слои, правила, раскладки клавиш и макросы будут заменены.
            Настройки приложения сохраняются.
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
                Внимание: у текущей раскладки есть несохранённые изменения.
              </div>
              <div>
                Если вы продолжите, они будут безвозвратно потеряны.
                Вернитесь и нажмите <b>«Сохранить текущую…»</b>, чтобы
                записать их в файл.
              </div>
            </div>
          </div>
        </div>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton variant="ghost" color="neutral" @click="cancelApply">
            Отмена
          </UButton>
          <UButton
            :color="isLayoutDirty ? 'error' : 'primary'"
            :loading="!!applying"
            @click="confirmApply"
          >
            {{ isLayoutDirty ? 'Потерять изменения и переключить' : 'Переключить' }}
          </UButton>
        </div>
      </template>
    </UModal>

    <!-- Save-as modal -->
    <UModal
      v-model:open="saveModalOpen"
      title="Сохранить раскладку"
    >
      <template #body>
        <div class="space-y-3">
          <UFormField label="Имя раскладки">
            <UInput
              v-model="saveName"
              placeholder="my-layout"
              autofocus
              @keyup.enter="performSave"
            />
          </UFormField>
          <p class="text-xs text-(--ui-text-muted)">
            Файл будет сохранён в
            <code class="break-all">{{ library.layoutsDir.value }}/{{ saveName || '…' }}.yaml</code>.
            Существующая раскладка с таким же именем будет перезаписана.
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
            Отмена
          </UButton>
          <UButton
            color="primary"
            icon="i-lucide-save"
            :loading="saveBusy"
            :disabled="!saveName.trim()"
            @click="performSave"
          >
            Сохранить
          </UButton>
        </div>
      </template>
    </UModal>

    <!-- Delete confirmation -->
    <UModal
      :open="!!deletePending"
      :title="`Удалить «${deletePending?.name ?? ''}»?`"
      @update:open="(v) => !v && (deletePending = null)"
    >
      <template #body>
        <p class="text-sm">
          Файл раскладки будет удалён с диска. Это действие необратимо.
          Текущая активная раскладка от этого не меняется.
        </p>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton
            variant="ghost"
            color="neutral"
            @click="deletePending = null"
          >
            Отмена
          </UButton>
          <UButton
            color="error"
            :loading="deleteBusy"
            @click="confirmDelete"
          >
            Удалить
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
