<script setup lang="ts">
import MacroEditorCard from '~/components/features/macros/MacroEditorCard.vue'
import SystemMacrosCard from '~/components/features/macros/SystemMacrosCard.vue'
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

        <MacroEditorCard
          v-for="(macro, macroIdx) in config.macros"
          :key="`${macro.id}:${macroIdx}`"
          :macro="macro"
          :id-error="idError(macro) ?? undefined"
          :usage="usage[macro.id] ?? []"
          :default-step-pause-ms="config.settings.defaultMacroStepPauseMs"
          :default-modifier-delay-ms="config.settings.defaultMacroModifierDelayMs"
          @remove="askRemove"
          @add-step="addStep"
          @move-step="moveStep"
          @remove-step="removeStep"
        />
      </div>
    </UCard>

    <SystemMacrosCard
      v-model:system-open="systemOpen"
      :usage="usage"
      :system-macros="SYSTEM_MACROS"
      @clone="cloneSystemMacro"
    />

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
