<script setup lang="ts">
import { type Macro } from '~/types/config'
import { randomId } from '~/utils/keys'

const { config } = useConfig()

function newMacroId(): string {
  let id: string
  do {
    id = randomId()
  } while (config.value.macros.some((m) => m.id === id))
  return id
}

function addMacro() {
  if (!Array.isArray(config.value.macros)) config.value.macros = []
  config.value.macros.push({
    id: newMacroId(),
    name: 'Новый макрос',
    steps: [],
    stepPauseMs: undefined,
    modifierDelayMs: undefined,
  })
}

function removeMacro(id: string) {
  config.value.macros = config.value.macros.filter((m) => m.id !== id)
}

function addStep(macro: Macro) {
  macro.steps.push({ id: randomId(), keystroke: '' })
}

function removeStep(macro: Macro, stepId: string) {
  macro.steps = macro.steps.filter((s) => s.id !== stepId)
}

function moveStep(macro: Macro, index: number, delta: number) {
  const next = index + delta
  if (next < 0 || next >= macro.steps.length) return
  const steps = macro.steps.slice()
  const [item] = steps.splice(index, 1)
  steps.splice(next, 0, item)
  macro.steps = steps
}

// --- Deletion confirmation -----------------------------------------------
const confirmOpen = ref(false)
const pendingDeleteId = ref<string | null>(null)

function askRemove(id: string) {
  pendingDeleteId.value = id
  confirmOpen.value = true
}

function confirmRemove() {
  if (pendingDeleteId.value) removeMacro(pendingDeleteId.value)
  pendingDeleteId.value = null
  confirmOpen.value = false
}

// --- Usage ---------------------------------------------------------------
const usage = computed(() => {
  const byMacro: Record<string, string[]> = {}
  const note = (id: string, where: string) => {
    if (!byMacro[id]) byMacro[id] = []
    byMacro[id].push(where)
  }
  const prefix = 'macro:'
  for (const r of config.value.rules) {
    if (r.tapAction?.startsWith(prefix)) {
      note(r.tapAction.slice(prefix.length), `rule ${r.key || '?'} (tap)`)
    }
  }
  for (const [lid, km] of Object.entries(config.value.layerKeymaps)) {
    for (const [code, action] of Object.entries(km.keys ?? {})) {
      if (action?.startsWith(prefix)) {
        note(action.slice(prefix.length), `${lid}.${code}`)
      }
    }
    for (const ex of km.extras ?? []) {
      if (ex.action?.startsWith(prefix)) {
        note(ex.action.slice(prefix.length), `${lid}.${ex.name || 'extra'}`)
      }
    }
  }
  return byMacro
})
</script>

<template>
  <div class="space-y-4">
    <UCard>
      <template #header>
        <div class="flex items-center justify-between gap-3">
          <div>
            <h2 class="font-semibold">Пользовательские макросы</h2>
            <p class="text-xs text-(--ui-text-muted) mt-1">
              Последовательность шагов. Каждый шаг — одно сочетание клавиш
              или системное действие. Шаги выполняются по очереди: предыдущее
              сочетание полностью отпускается, выдерживается пауза, затем
              нажимается следующее.
            </p>
          </div>
          <UButton icon="i-lucide-plus" size="sm" @click="addMacro">
            Новый макрос
          </UButton>
        </div>
      </template>

      <div class="space-y-4">
        <div
          v-if="config.macros.length === 0"
          class="text-sm text-(--ui-text-muted)"
        >
          Нет ни одного макроса. Нажмите «Новый макрос», чтобы создать
          первый.
        </div>

        <div
          v-for="macro in config.macros"
          :key="macro.id"
          class="rounded-md border border-(--ui-border) bg-(--ui-bg-muted) p-4 space-y-4"
        >
          <div class="grid grid-cols-1 md:grid-cols-[1fr_1fr_auto] gap-3">
            <UFormField>
              <template #label>
                <FieldLabel
                  label="Имя"
                  hint="Человекочитаемое имя макроса, показывается в списках выбора."
                />
              </template>
              <UInput
                v-model="macro.name"
                placeholder="Название макроса"
                class="w-full"
              />
            </UFormField>
            <UFormField>
              <template #label>
                <FieldLabel
                  label="ID"
                  hint="Уникальный идентификатор для ссылки вида macro:<id>. Изменяйте только если не используете этот макрос."
                />
              </template>
              <UInput
                v-model="macro.id"
                class="w-full font-mono"
                placeholder="id"
              />
            </UFormField>
            <div class="flex items-end">
              <UButton
                icon="i-lucide-trash-2"
                color="error"
                variant="ghost"
                square
                aria-label="Удалить макрос"
                @click="askRemove(macro.id)"
              />
            </div>
          </div>

          <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
            <UFormField>
              <template #label>
                <FieldLabel
                  label="Пауза между шагами"
                  hint="Индивидуальная пауза между шагами этого макроса. По умолчанию — значение из настроек."
                />
              </template>
              <OverridableNumberField
                v-model="macro.stepPauseMs"
                :default-value="config.settings.defaultMacroStepPauseMs"
                suffix="мс"
              />
            </UFormField>
            <UFormField>
              <template #label>
                <FieldLabel
                  label="Задержка модификатора"
                  hint="Время между нажатием модификатора и основной клавиши внутри одного шага. По умолчанию — значение из настроек."
                />
              </template>
              <OverridableNumberField
                v-model="macro.modifierDelayMs"
                :default-value="config.settings.defaultMacroModifierDelayMs"
                suffix="мс"
              />
            </UFormField>
          </div>

          <div
            v-if="usage[macro.id] && usage[macro.id].length"
            class="flex flex-wrap gap-1 text-xs"
          >
            <span class="text-(--ui-text-muted)">Используется в:</span>
            <UBadge
              v-for="(u, idx) in usage[macro.id]"
              :key="idx"
              color="neutral"
              variant="subtle"
              class="font-mono"
            >
              {{ u }}
            </UBadge>
          </div>

          <div class="border-t border-(--ui-border) pt-3">
            <div class="flex items-center justify-between mb-2">
              <div class="text-sm font-medium">Шаги</div>
              <UButton
                size="xs"
                icon="i-lucide-plus"
                variant="outline"
                @click="addStep(macro)"
              >
                Добавить шаг
              </UButton>
            </div>

            <div
              v-if="macro.steps.length === 0"
              class="text-sm text-(--ui-text-muted) italic"
            >
              Пока нет шагов. Каждый шаг — сочетание клавиш или действие.
            </div>

            <div v-else class="space-y-1.5">
              <div
                v-for="(step, idx) in macro.steps"
                :key="step.id"
                class="grid grid-cols-[2rem_1fr_auto_auto_auto] gap-2 items-center"
              >
                <div
                  class="text-xs text-(--ui-text-muted) font-mono text-right"
                >
                  #{{ idx + 1 }}
                </div>
                <ActionPickerModal
                  v-model="step.keystroke"
                  placeholder="Ctrl+C или выберите действие"
                />
                <UButton
                  icon="i-lucide-chevron-up"
                  size="xs"
                  variant="ghost"
                  color="neutral"
                  square
                  :disabled="idx === 0"
                  aria-label="Вверх"
                  @click="moveStep(macro, idx, -1)"
                />
                <UButton
                  icon="i-lucide-chevron-down"
                  size="xs"
                  variant="ghost"
                  color="neutral"
                  square
                  :disabled="idx === macro.steps.length - 1"
                  aria-label="Вниз"
                  @click="moveStep(macro, idx, 1)"
                />
                <UButton
                  icon="i-lucide-trash-2"
                  size="xs"
                  variant="ghost"
                  color="error"
                  square
                  aria-label="Удалить шаг"
                  @click="removeStep(macro, step.id)"
                />
              </div>
            </div>
          </div>

          <div class="text-xs text-(--ui-text-muted)">
            Чтобы назначить макрос на клавишу, выберите действие
            <code class="font-mono">macro:{{ macro.id }}</code>
            во вкладках «Слои» или «Раскладка».
          </div>
        </div>
      </div>
    </UCard>

    <UModal v-model:open="confirmOpen" title="Удалить макрос?">
      <template #body>
        <p class="text-sm">
          Макрос будет удалён. Ссылки <code>macro:{{ pendingDeleteId }}</code>
          перестанут работать.
        </p>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton variant="ghost" color="neutral" @click="confirmOpen = false">
            Отмена
          </UButton>
          <UButton color="error" icon="i-lucide-trash-2" @click="confirmRemove">
            Удалить
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
