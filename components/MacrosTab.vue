<script setup lang="ts">
import { type Macro } from '~/types/config'
import { randomId } from '~/utils/keys'
import { SYSTEM_MACROS, systemMacroById, type SystemMacro } from '~/utils/systemMacros'

const { config } = useConfig()
const { t } = useI18n()

// Regex for a well-formed macro id: letters, digits, underscore and dash,
// 1..64 chars. Keeps `macro:<id>` references unambiguous.
const ID_RE = /^[A-Za-z0-9_-]{1,64}$/

function newMacroId(base?: string): string {
  if (base && !config.value.macros.some((m) => m.id === base)) return base
  if (base) {
    // Try base, base2, base3 ... before falling back to random.
    for (let i = 2; i < 1000; i++) {
      const candidate = `${base}${i}`
      if (!config.value.macros.some((m) => m.id === candidate)) return candidate
    }
  }
  let id: string
  do {
    id = randomId()
  } while (config.value.macros.some((m) => m.id === id))
  return id
}

function cloneSystemMacro(sys: SystemMacro) {
  if (!Array.isArray(config.value.macros)) config.value.macros = []
  // Never reuse `sys.id` — it belongs to the system catalog and an equal
  // user id is rejected by validation. Start from `<sys.id>Copy`.
  config.value.macros.push({
    id: newMacroId(`${sys.id}Copy`),
    name: `${sys.name} ${t('macros.copySuffix')}`,
    steps: sys.steps.map((s) => ({ id: randomId(), keystroke: s.keystroke })),
    stepPauseMs: undefined,
    modifierDelayMs: undefined,
  })
}

function stepsPreview(sys: SystemMacro): string {
  return sys.steps.map((s) => s.keystroke).join(' → ')
}

const systemOpen = ref(false)

function addMacro() {
  if (!Array.isArray(config.value.macros)) config.value.macros = []
  config.value.macros.push({
    id: newMacroId(),
    name: t('macros.defaultName'),
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

// --- Id validation -------------------------------------------------------
// Count how many user macros currently use each id — anything > 1 is a
// duplicate and must be flagged. Computed once per config change.
const idCounts = computed<Record<string, number>>(() => {
  const counts: Record<string, number> = {}
  for (const m of config.value.macros) {
    counts[m.id] = (counts[m.id] ?? 0) + 1
  }
  return counts
})

function idError(macro: Macro): string | null {
  const raw = macro.id ?? ''
  if (raw.trim() === '') return t('macros.idErrors.empty')
  if (!ID_RE.test(raw)) {
    return t('macros.idErrors.format')
  }
  if ((idCounts.value[raw] ?? 0) > 1) {
    return t('macros.idErrors.dupUser')
  }
  const sys = systemMacroById(raw)
  if (sys) {
    return t('macros.idErrors.dupSystem', { name: sys.name })
  }
  return null
}

// Disable "Новый макрос" / "Создать на основе" while the user has an
// unresolved id conflict — otherwise a new auto-id may land on top of a
// duplicate the user hasn't fixed yet.
const hasIdErrors = computed(() =>
  config.value.macros.some((m) => idError(m) !== null),
)

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
            <h2 class="font-semibold">{{ $t('macros.title') }}</h2>
            <p class="text-xs text-(--ui-text-muted) mt-1">
              {{ $t('macros.subtitle') }}
            </p>
          </div>
          <UButton
            icon="i-lucide-plus"
            size="sm"
            :disabled="hasIdErrors"
            :title="
              hasIdErrors
                ? $t('macros.addDisabled')
                : undefined
            "
            @click="addMacro"
          >
            {{ $t('macros.addBtn') }}
          </UButton>
        </div>
      </template>

      <div class="space-y-4">
        <div
          v-if="config.macros.length === 0"
          class="text-sm text-(--ui-text-muted)"
        >
          {{ $t('macros.empty') }}
        </div>

        <div
          v-for="(macro, macroIdx) in config.macros"
          :key="macroIdx"
          class="rounded-md border border-(--ui-border) bg-(--ui-bg-muted) p-4 space-y-4"
        >
          <div class="grid grid-cols-1 md:grid-cols-[1fr_1fr_auto] gap-3">
            <UFormField>
              <template #label>
                <FieldLabel
                  :label="$t('macros.nameLabel')"
                  :hint="$t('macros.nameHint')"
                />
              </template>
              <UInput
                v-model="macro.name"
                :placeholder="$t('macros.namePh')"
                class="w-full"
              />
            </UFormField>
            <UFormField :error="idError(macro) ?? undefined">
              <template #label>
                <FieldLabel
                  :label="$t('macros.idLabel')"
                  :hint="$t('macros.idHint')"
                />
              </template>
              <UInput
                v-model="macro.id"
                :color="idError(macro) ? 'error' : undefined"
                :highlight="!!idError(macro)"
                class="w-full font-mono"
                :placeholder="$t('macros.idPh')"
              />
            </UFormField>
            <div class="flex items-end">
              <UButton
                icon="i-lucide-trash-2"
                color="error"
                variant="ghost"
                square
                :aria-label="$t('macros.deleteMacro')"
                @click="askRemove(macro.id)"
              />
            </div>
          </div>

          <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
            <UFormField>
              <template #label>
                <FieldLabel
                  :label="$t('macros.stepPauseLabel')"
                  :hint="$t('macros.stepPauseHint')"
                />
              </template>
              <OverridableNumberField
                v-model="macro.stepPauseMs"
                :default-value="config.settings.defaultMacroStepPauseMs"
                :suffix="$t('common.ms')"
              />
            </UFormField>
            <UFormField>
              <template #label>
                <FieldLabel
                  :label="$t('macros.modDelayLabel')"
                  :hint="$t('macros.modDelayHint')"
                />
              </template>
              <OverridableNumberField
                v-model="macro.modifierDelayMs"
                :default-value="config.settings.defaultMacroModifierDelayMs"
                :suffix="$t('common.ms')"
              />
            </UFormField>
          </div>

          <div
            v-if="usage[macro.id] && usage[macro.id].length"
            class="flex flex-wrap gap-1 text-xs"
          >
            <span class="text-(--ui-text-muted)">{{ $t('macros.usedIn') }}</span>
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
              <div class="text-sm font-medium">{{ $t('macros.steps') }}</div>
              <UButton
                size="xs"
                icon="i-lucide-plus"
                variant="outline"
                @click="addStep(macro)"
              >
                {{ $t('macros.addStep') }}
              </UButton>
            </div>

            <div
              v-if="macro.steps.length === 0"
              class="text-sm text-(--ui-text-muted) italic"
            >
              {{ $t('macros.stepsEmpty') }}
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
                  :placeholder="$t('macros.stepPh')"
                />
                <UButton
                  icon="i-lucide-chevron-up"
                  size="xs"
                  variant="ghost"
                  color="neutral"
                  square
                  :disabled="idx === 0"
                  :aria-label="$t('macros.moveUp')"
                  @click="moveStep(macro, idx, -1)"
                />
                <UButton
                  icon="i-lucide-chevron-down"
                  size="xs"
                  variant="ghost"
                  color="neutral"
                  square
                  :disabled="idx === macro.steps.length - 1"
                  :aria-label="$t('macros.moveDown')"
                  @click="moveStep(macro, idx, 1)"
                />
                <UButton
                  icon="i-lucide-trash-2"
                  size="xs"
                  variant="ghost"
                  color="error"
                  square
                  :aria-label="$t('macros.deleteStep')"
                  @click="removeStep(macro, step.id)"
                />
              </div>
            </div>
          </div>

          <div class="text-xs text-(--ui-text-muted)">
            <i18n-t keypath="macros.assignHint" tag="span">
              <template #ref>
                <code class="font-mono">macro:{{ macro.id }}</code>
              </template>
            </i18n-t>
          </div>
        </div>
      </div>
    </UCard>

    <UCard>
      <template #header>
        <button
          type="button"
          class="flex items-center justify-between gap-3 w-full text-left"
          :aria-expanded="systemOpen"
          @click="systemOpen = !systemOpen"
        >
          <div>
            <h2 class="font-semibold flex items-center gap-2">
              <UIcon
                :name="
                  systemOpen ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'
                "
                class="text-(--ui-text-muted)"
              />
              {{ $t('macros.systemTitle') }}
              <UBadge color="neutral" variant="subtle" size="sm">
                {{ SYSTEM_MACROS.length }}
              </UBadge>
            </h2>
            <p class="text-xs text-(--ui-text-muted) mt-1">
              {{ $t('macros.systemSub') }}
            </p>
          </div>
        </button>
      </template>

      <div v-show="systemOpen">
        <div
          v-if="SYSTEM_MACROS.length === 0"
          class="text-sm text-(--ui-text-muted)"
        >
          {{ $t('macros.systemEmpty') }}
        </div>

        <div v-else class="overflow-x-auto">
          <table class="w-full text-sm">
            <thead>
              <tr
                class="text-left text-xs text-(--ui-text-muted) border-b border-(--ui-border)"
              >
                <th class="py-2 pr-3 font-medium">{{ $t('macros.colId') }}</th>
                <th class="py-2 pr-3 font-medium">{{ $t('macros.colName') }}</th>
                <th class="py-2 pr-3 font-medium">{{ $t('macros.colSteps') }}</th>
                <th class="py-2 pr-3 font-medium w-px"></th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="sys in SYSTEM_MACROS"
                :key="sys.id"
                class="border-b border-(--ui-border) last:border-b-0 align-top"
              >
                <td class="py-2 pr-3 font-mono text-xs whitespace-nowrap">
                  {{ sys.id }}
                </td>
                <td class="py-2 pr-3">
                  <div>{{ sys.name }}</div>
                  <div
                    v-if="sys.description"
                    class="text-xs text-(--ui-text-muted) mt-0.5"
                  >
                    {{ sys.description }}
                  </div>
                </td>
                <td class="py-2 pr-3">
                  <code class="text-xs font-mono text-(--ui-text-muted)">
                    {{ stepsPreview(sys) }}
                  </code>
                  <div
                    v-if="usage[sys.id] && usage[sys.id].length"
                    class="flex flex-wrap gap-1 mt-1"
                  >
                    <UBadge
                      v-for="(u, idx) in usage[sys.id]"
                      :key="idx"
                      color="neutral"
                      variant="subtle"
                      size="sm"
                      class="font-mono"
                    >
                      {{ u }}
                    </UBadge>
                  </div>
                </td>
                <td class="py-2 pr-3 whitespace-nowrap">
                  <UButton
                    size="xs"
                    variant="outline"
                    icon="i-lucide-copy-plus"
                    @click="cloneSystemMacro(sys)"
                  >
                    {{ $t('macros.cloneBtn') }}
                  </UButton>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </UCard>

    <UModal v-model:open="confirmOpen" :title="$t('macros.confirmDeleteTitle')">
      <template #body>
        <p class="text-sm">
          <i18n-t keypath="macros.confirmDeleteBody" tag="span">
            <template #ref>
              <code>macro:{{ pendingDeleteId }}</code>
            </template>
          </i18n-t>
        </p>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton variant="ghost" color="neutral" @click="confirmOpen = false">
            {{ $t('common.cancel') }}
          </UButton>
          <UButton color="error" icon="i-lucide-trash-2" @click="confirmRemove">
            {{ $t('common.delete') }}
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
