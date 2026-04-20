<script setup lang="ts">
import { randomId } from '~/utils/keys'

const { config } = useConfig()

const layerOptions = computed(() =>
  config.value.layers
    .filter((l) => l.id !== 'base')
    .map((l) => ({ label: l.name, value: l.id })),
)

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

// --- New-layer modal -----------------------------------------------------
const newLayerOpen = ref(false)
const newLayerName = ref('')
const newLayerDescription = ref('')
const newLayerForRuleId = ref<string | null>(null)

function openNewLayer(ruleId: string) {
  newLayerForRuleId.value = ruleId
  newLayerName.value = ''
  newLayerDescription.value = ''
  newLayerOpen.value = true
}

function confirmNewLayer() {
  const name = newLayerName.value.trim()
  if (!name) return
  const id = randomId()
  config.value.layers.push({
    id,
    name,
    description: newLayerDescription.value.trim() || undefined,
  })
  config.value.layerKeymaps[id] = { keys: {}, extras: [] }
  if (newLayerForRuleId.value) {
    const rule = config.value.rules.find(
      (r) => r.id === newLayerForRuleId.value,
    )
    if (rule) rule.layerId = id
  }
  newLayerOpen.value = false
}
</script>

<template>
  <div class="space-y-6">
    <UCard>
      <template #header>
        <div class="flex items-center justify-between">
          <div>
            <h2 class="font-semibold">Слои и их триггеры</h2>
            <p class="text-xs text-(--ui-text-muted) mt-1">
              На каждой строке — правило: какая физическая клавиша активирует
              слой при удержании и какое действие срабатывает на короткое
              нажатие.
            </p>
          </div>
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
          class="grid grid-cols-[1fr_1fr_1fr_auto_auto] gap-3 items-start p-3 rounded-md bg-(--ui-bg-muted)"
        >
          <UFormField>
            <template #label>
              <FieldLabel
                label="Клавиша"
                hint="Физическая клавиша, которая будет активировать слой при удержании или выполнять tap-действие при коротком нажатии."
              />
            </template>
            <ActionPickerModal
              v-model="rule.key"
              key-only
              placeholder="выберите клавишу"
            />
          </UFormField>

          <UFormField>
            <template #label>
              <FieldLabel
                label="Слой (hold)"
                hint="Слой, который активируется пока клавиша удерживается. Пусто — клавиша работает только как tap."
              />
            </template>
            <div class="flex gap-1">
              <USelectMenu
                v-model="rule.layerId"
                :items="layerOptions"
                value-key="value"
                placeholder="— none —"
                class="flex-1 min-w-0"
              />
              <UButton
                v-if="rule.layerId"
                icon="i-lucide-x"
                variant="ghost"
                color="neutral"
                square
                aria-label="Очистить слой"
                @click="rule.layerId = ''"
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
          </UFormField>

          <UFormField>
            <template #label>
              <FieldLabel
                label="Tap action"
                hint="Действие при коротком нажатии клавиши (отпущена до истечения hold-таймаута)."
              />
            </template>
            <ActionPickerModal
              v-model="rule.tapAction"
              allow-empty
              placeholder="нет действия"
            />
          </UFormField>

          <UFormField>
            <template #label>
              <FieldLabel
                label="Hold ms"
                hint="Индивидуальный таймаут удержания для этого правила. По умолчанию используется значение из настроек."
              />
            </template>
            <OverridableNumberField
              v-model="rule.holdTimeoutMs"
              :default-value="config.settings.defaultHoldTimeoutMs"
              suffix="мс"
            />
          </UFormField>

          <div class="pt-6">
            <UButton
              icon="i-lucide-trash-2"
              color="error"
              variant="ghost"
              square
              aria-label="Удалить правило"
              @click="removeRule(rule.id)"
            />
          </div>
        </div>
      </div>
    </UCard>

    <UModal v-model:open="newLayerOpen" title="Новый слой">
      <template #body>
        <div class="space-y-3">
          <UFormField label="Имя слоя">
            <UInput
              v-model="newLayerName"
              autofocus
              placeholder="Например: Навигация"
              class="w-full"
              @keydown.enter="confirmNewLayer"
            />
          </UFormField>
          <UFormField label="Описание (необязательно)">
            <UTextarea
              v-model="newLayerDescription"
              placeholder="Коротко: для чего нужен этот слой"
              class="w-full"
              :rows="2"
            />
          </UFormField>
        </div>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton
            variant="ghost"
            color="neutral"
            @click="newLayerOpen = false"
          >
            Отмена
          </UButton>
          <UButton
            icon="i-lucide-check"
            :disabled="!newLayerName.trim()"
            @click="confirmNewLayer"
          >
            Создать
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
