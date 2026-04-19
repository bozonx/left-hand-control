<script setup lang="ts">
import { ALL_KEYS, keyLabel, randomId } from '~/utils/keys'

const { config } = useConfig()

const keyOptions = computed(() =>
  ALL_KEYS.map((k) => ({ label: `${k.label}  (${k.code})`, value: k.code })),
)

const layerOptions = computed(() => [
  { label: '— none —', value: '' },
  ...config.value.layers
    .filter((l) => l.id !== 'base')
    .map((l) => ({ label: l.name, value: l.id })),
])

function addRule() {
  config.value.rules.push({
    id: randomId(),
    key: '',
    layerId: '',
    tapAction: '',
    holdTimeoutMs: undefined,
  })
}

function removeRule(id: string) {
  config.value.rules = config.value.rules.filter((r) => r.id !== id)
}

// --- New-layer inline creation -------------------------------------------
const newLayerOpen = ref<Record<string, boolean>>({})
const newLayerName = ref<Record<string, string>>({})

function openNewLayer(ruleId: string) {
  newLayerOpen.value[ruleId] = true
  newLayerName.value[ruleId] = ''
}

function confirmNewLayer(ruleId: string) {
  const name = (newLayerName.value[ruleId] ?? '').trim()
  if (!name) return
  const id = randomId()
  config.value.layers.push({ id, name })
  config.value.layerKeymaps[id] = { keys: {}, extras: [] }
  const rule = config.value.rules.find((r) => r.id === ruleId)
  if (rule) rule.layerId = id
  newLayerOpen.value[ruleId] = false
}
</script>

<template>
  <div class="space-y-6">
    <UCard>
      <template #header>
        <div class="flex items-center justify-between">
          <h2 class="font-semibold">Назначение слоёв клавишам</h2>
          <UButton icon="i-lucide-plus" size="sm" @click="addRule">
            Добавить правило
          </UButton>
        </div>
      </template>

      <div v-if="config.rules.length === 0" class="text-sm text-(--ui-text-muted)">
        Пока нет правил. Нажмите «Добавить правило», чтобы назначить слой на
        клавишу или определить действие одиночного нажатия.
      </div>

      <div v-else class="space-y-3">
        <div
          v-for="rule in config.rules"
          :key="rule.id"
          class="grid grid-cols-[1fr_1fr_1fr_140px_auto] gap-3 items-start p-3 rounded-md bg-(--ui-bg-muted)"
        >
          <UFormField label="Клавиша">
            <USelectMenu
              v-model="rule.key"
              :items="keyOptions"
              value-key="value"
              placeholder="выберите клавишу"
              class="w-full"
            />
          </UFormField>

          <UFormField label="Слой (hold)">
            <div class="flex gap-2">
              <USelectMenu
                v-model="rule.layerId"
                :items="layerOptions"
                value-key="value"
                class="flex-1 min-w-0"
              />
              <UButton
                icon="i-lucide-plus"
                variant="outline"
                color="neutral"
                square
                aria-label="Создать слой"
                @click="openNewLayer(rule.id)"
              />
            </div>
            <div
              v-if="newLayerOpen[rule.id]"
              class="mt-2 flex gap-2"
            >
              <UInput
                v-model="newLayerName[rule.id]"
                placeholder="Имя нового слоя"
                class="flex-1"
                @keydown.enter="confirmNewLayer(rule.id)"
              />
              <UButton size="sm" @click="confirmNewLayer(rule.id)">OK</UButton>
              <UButton
                size="sm"
                color="neutral"
                variant="ghost"
                @click="newLayerOpen[rule.id] = false"
              >
                ✕
              </UButton>
            </div>
          </UFormField>

          <UFormField
            label="Tap action"
            help="Действие на одиночное нажатие"
          >
            <UInput
              v-model="rule.tapAction"
              placeholder="например: Escape"
            />
          </UFormField>

          <UFormField
            label="Hold ms"
            :help="`def ${config.settings.defaultHoldTimeoutMs}`"
          >
            <UInput
              v-model.number="rule.holdTimeoutMs"
              type="number"
              min="0"
              placeholder="—"
            />
          </UFormField>

          <div class="pt-6">
            <UButton
              icon="i-lucide-trash-2"
              color="error"
              variant="ghost"
              square
              :aria-label="`Удалить правило ${keyLabel(rule.key)}`"
              @click="removeRule(rule.id)"
            />
          </div>
        </div>
      </div>
    </UCard>

    <UCard>
      <template #header>
        <h2 class="font-semibold">
          Определение одиночного нажатия vs удержания слоя
        </h2>
      </template>
      <p class="text-sm text-(--ui-text-muted) mb-3">
        Время ожидания отпускания клавиши. Если клавиша отпущена до истечения
        таймаута — срабатывает <code>tap action</code>. Если удерживается
        дольше — активируется назначенный слой.
      </p>
      <UFormField label="Таймаут по умолчанию, мс">
        <UInput
          v-model.number="config.settings.defaultHoldTimeoutMs"
          type="number"
          min="0"
          class="w-40"
        />
      </UFormField>
    </UCard>
  </div>
</template>
