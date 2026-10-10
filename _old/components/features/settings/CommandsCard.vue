<script setup lang="ts">
import { parseCommandRef, type Command } from '~/types/config'

interface Assignment {
  layoutId?: string
  command: Command
  usage: string[]
}

const { config, flush, applyPreset, isLayoutDirty } = useConfig()
const { usage } = useActionUsage(parseCommandRef)
const library = useLayoutLibrary()
const { t } = useI18n()
const assignments = ref<Assignment[]>([])
const error = ref('')
const editing = ref(false)
const action = ref<string | null>('')
let generation = 0

async function refresh() {
  const request = ++generation
  const current = config.value.commands.flatMap((command) => {
    const places = usage.value[command.id] ?? []
    return places.length ? [{ layoutId: config.value.settings.currentLayoutId, command, usage: places }] : []
  })
  assignments.value = current
  if (!config.value.settings.commandsEnabled) return
  try {
    await nextTick()
    await flush()
    const tauri = await useTauri()
    if (!tauri) return
    const rows = await tauri.invoke<Assignment[]>('get_command_assignments')
    if (request === generation) assignments.value = rows
    error.value = ''
  } catch (reason) {
    if (request === generation) error.value = String(reason)
  }
}

async function edit(row: Assignment) {
  if (row.layoutId !== config.value.settings.currentLayoutId) {
    if (isLayoutDirty.value) {
      error.value = t('commands.saveLayoutFirst')
      return
    }
    const preset = row.layoutId ? await library.loadPreset(row.layoutId) : null
    if (!preset) return
    await applyPreset(preset, row.layoutId, false)
  }
  error.value = ''
  action.value = `cmd:${row.command.id}`
  editing.value = true
}

watch(() => [config.value.settings.commandsEnabled, config.value.commands, config.value.rules, config.value.layerKeymaps, config.value.macros, config.value.quickActions, config.value.settings.currentLayoutId], refresh, { deep: true, immediate: true })
</script>

<template>
  <UCard>
    <UCheckbox v-model="config.settings.commandsEnabled" :label="$t('commands.enabled')" />
    <div v-if="config.settings.commandsEnabled" class="mt-4 space-y-3">
      <p v-if="error" class="text-sm text-(--ui-error)">{{ error }}</p>
      <p v-if="assignments.length === 0" class="text-sm text-(--ui-text-muted)">{{ $t('commands.usedEmpty') }}</p>
      <div v-for="row in assignments" :key="`${row.layoutId}:${row.command.id}`" class="rounded-lg border border-(--ui-border) p-3 space-y-2">
        <p class="font-medium">{{ row.command.name || row.command.linux }}</p>
        <p class="whitespace-pre-wrap font-mono text-sm text-(--ui-text-muted)">{{ row.command.linux }}</p>
        <p class="text-xs text-(--ui-text-muted)">{{ row.layoutId || $t('app.customLayout') }}</p>
        <div class="flex flex-wrap gap-2">
          <UButton v-for="place in row.usage" :key="place" color="neutral" variant="outline" size="xs" @click="edit(row)">{{ place }}</UButton>
        </div>
      </div>
    </div>
    <ActionPickerModal v-model="action" v-model:open="editing" hide-trigger require-value command-only @apply="refresh" />
  </UCard>
</template>
