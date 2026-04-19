<script setup lang="ts">
import { type Macro } from '~/types/config'
import { randomId } from '~/utils/keys'

const { config } = useConfig()

function newMacroId(): string {
  // Short + unique-within-list id. Users can rename via name but id is stable
  // so references `macro:<id>` don't break.
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

// Usage detection: find where each macro is referenced (rules / keymaps /
// extras). Purely informational — rendered as a small badge.
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
              Макрос — последовательность шагов, каждый шаг это одно сочетание
              клавиш. Шаги выполняются по очереди: предыдущее сочетание
              полностью отпускается, выдерживается пауза, затем нажимается
              следующее. В будущем сюда можно будет добавить вызов консольной
              команды или системного действия.
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
          class="rounded-md border border-(--ui-border) bg-(--ui-bg-muted) p-3 space-y-3"
        >
          <div class="flex flex-wrap items-end gap-3">
            <UFormField label="Имя" class="flex-1 min-w-[200px]">
              <UInput v-model="macro.name" placeholder="Название макроса" />
            </UFormField>
            <UFormField label="ID (для macro:<id>)">
              <UInput
                v-model="macro.id"
                class="w-40 font-mono"
                placeholder="id"
              />
            </UFormField>
            <UFormField
              label="Пауза между шагами, мс"
              :help="`def ${config.settings.defaultMacroStepPauseMs}`"
            >
              <UInput
                v-model.number="macro.stepPauseMs"
                type="number"
                min="0"
                class="w-28"
                placeholder="—"
              />
            </UFormField>
            <UFormField
              label="Задержка модификатора, мс"
              :help="`def ${config.settings.defaultMacroModifierDelayMs}`"
            >
              <UInput
                v-model.number="macro.modifierDelayMs"
                type="number"
                min="0"
                class="w-28"
                placeholder="—"
              />
            </UFormField>
            <UButton
              icon="i-lucide-trash-2"
              color="error"
              variant="ghost"
              square
              aria-label="Удалить макрос"
              @click="removeMacro(macro.id)"
            />
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

          <div>
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
              Пока нет шагов. Каждый шаг — сочетание клавиш
              (<code>Ctrl+C</code>, <code>Shift+End</code>, <code>Enter</code>).
            </div>

            <div v-else class="space-y-1.5">
              <div
                v-for="(step, idx) in macro.steps"
                :key="step.id"
                class="grid grid-cols-[2.5rem_1fr_auto_auto_auto] gap-2 items-center"
              >
                <div
                  class="text-xs text-(--ui-text-muted) font-mono text-right"
                >
                  #{{ idx + 1 }}
                </div>
                <ActionPicker
                  v-model="step.keystroke"
                  placeholder="Ctrl+C"
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
            во вкладках «Правила» или «Keymap».
          </div>
        </div>
      </div>
    </UCard>

    <UCard>
      <template #header>
        <h2 class="font-semibold">Глобальные задержки макросов</h2>
      </template>
      <div class="flex flex-wrap gap-6">
        <UFormField
          label="Пауза между шагами, мс"
          help="Используется, когда шаг макроса не задаёт свою паузу."
        >
          <UInput
            v-model.number="config.settings.defaultMacroStepPauseMs"
            type="number"
            min="0"
            class="w-40"
          />
        </UFormField>
        <UFormField
          label="Задержка модификатора, мс"
          help="Время между нажатием модификатора и основной клавиши внутри одного шага."
        >
          <UInput
            v-model.number="config.settings.defaultMacroModifierDelayMs"
            type="number"
            min="0"
            class="w-40"
          />
        </UFormField>
      </div>
    </UCard>
  </div>
</template>
