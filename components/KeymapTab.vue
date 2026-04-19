<script setup lang="ts">
import { LEFT_HAND_ROWS, RIGHT_HAND_ROWS, randomId } from '~/utils/keys'
import { BASE_LAYER_ID } from '~/types/config'

const { config } = useConfig()

const selectedLayerId = ref<string>(BASE_LAYER_ID)

watch(
  () => config.value.layers.map((l) => l.id).join(','),
  () => {
    if (!config.value.layers.some((l) => l.id === selectedLayerId.value)) {
      selectedLayerId.value = BASE_LAYER_ID
    }
  },
)

const layerItems = computed(() =>
  config.value.layers.map((l) => ({ label: l.name, value: l.id })),
)

const currentKeymap = computed(() => {
  const id = selectedLayerId.value
  if (!config.value.layerKeymaps[id]) {
    config.value.layerKeymaps[id] = { keys: {}, extras: [] }
  }
  return config.value.layerKeymaps[id]
})

// --- Edit modal ----------------------------------------------------------
const editOpen = ref(false)
const editKeyCode = ref('')
const editKeyLabel = ref('')
const editAction = ref('')

function openEdit(code: string, label: string) {
  editKeyCode.value = code
  editKeyLabel.value = label
  editAction.value = currentKeymap.value.keys[code] ?? ''
  editOpen.value = true
}

function saveEdit(action: string) {
  if (action) currentKeymap.value.keys[editKeyCode.value] = action
  else delete currentKeymap.value.keys[editKeyCode.value]
}

function clearEdit() {
  delete currentKeymap.value.keys[editKeyCode.value]
}

// --- Extra keys ----------------------------------------------------------
function addExtra() {
  currentKeymap.value.extras.push({
    id: randomId(),
    name: '',
    action: '',
  })
}

function removeExtra(id: string) {
  currentKeymap.value.extras = currentKeymap.value.extras.filter(
    (e) => e.id !== id,
  )
}

// --- Layer management ----------------------------------------------------
const renameOpen = ref(false)
const renameDraft = ref('')
function openRename() {
  const l = config.value.layers.find((l) => l.id === selectedLayerId.value)
  renameDraft.value = l?.name ?? ''
  renameOpen.value = true
}
function confirmRename() {
  const l = config.value.layers.find((l) => l.id === selectedLayerId.value)
  if (l) l.name = renameDraft.value.trim() || l.name
  renameOpen.value = false
}

function deleteLayer() {
  if (selectedLayerId.value === BASE_LAYER_ID) return
  const id = selectedLayerId.value
  config.value.layers = config.value.layers.filter((l) => l.id !== id)
  delete config.value.layerKeymaps[id]
  config.value.rules = config.value.rules.map((r) =>
    r.layerId === id ? { ...r, layerId: '' } : r,
  )
  selectedLayerId.value = BASE_LAYER_ID
}

const newLayerOpen = ref(false)
const newLayerName = ref('')
function openNewLayer() {
  newLayerName.value = ''
  newLayerOpen.value = true
}
function confirmNewLayer() {
  const name = newLayerName.value.trim()
  if (!name) return
  const id = randomId()
  config.value.layers.push({ id, name })
  config.value.layerKeymaps[id] = { keys: {}, extras: [] }
  selectedLayerId.value = id
  newLayerOpen.value = false
}
</script>

<template>
  <div class="space-y-4">
    <UCard>
      <div class="flex flex-wrap items-center gap-3">
        <UFormField label="Слой" class="flex-1 min-w-[220px]">
          <USelectMenu
            v-model="selectedLayerId"
            :items="layerItems"
            value-key="value"
            class="w-full"
          />
        </UFormField>
        <div class="flex gap-2 pt-6">
          <UButton icon="i-lucide-plus" size="sm" @click="openNewLayer">
            Новый слой
          </UButton>
          <UButton
            icon="i-lucide-pencil"
            size="sm"
            color="neutral"
            variant="outline"
            :disabled="selectedLayerId === 'base'"
            @click="openRename"
          >
            Переименовать
          </UButton>
          <UButton
            icon="i-lucide-trash-2"
            size="sm"
            color="error"
            variant="outline"
            :disabled="selectedLayerId === 'base'"
            @click="deleteLayer"
          >
            Удалить
          </UButton>
        </div>
      </div>
    </UCard>

    <!-- Row 1 : keyboard grid -->
    <UCard>
      <template #header>
        <h2 class="font-semibold">Клавиатура</h2>
      </template>
      <div class="grid grid-cols-1 xl:grid-cols-2 gap-6">
        <section>
          <div
            class="text-xs uppercase tracking-wide text-(--ui-text-muted) mb-2"
          >
            Левая рука
          </div>
          <div class="space-y-1.5">
            <div
              v-for="(kbRow, i) in LEFT_HAND_ROWS"
              :key="`l-${i}`"
              class="flex flex-wrap gap-1.5"
            >
              <KeyCap
                v-for="k in kbRow"
                :key="k.code"
                :label="k.label"
                :action="currentKeymap.keys[k.code]"
                @edit="openEdit(k.code, k.label)"
              />
            </div>
          </div>
        </section>
        <section>
          <div
            class="text-xs uppercase tracking-wide text-(--ui-text-muted) mb-2"
          >
            Правая рука
          </div>
          <div class="space-y-1.5">
            <div
              v-for="(kbRow, i) in RIGHT_HAND_ROWS"
              :key="`r-${i}`"
              class="flex flex-wrap gap-1.5"
            >
              <KeyCap
                v-for="k in kbRow"
                :key="k.code"
                :label="k.label"
                :action="currentKeymap.keys[k.code]"
                @edit="openEdit(k.code, k.label)"
              />
            </div>
          </div>
        </section>
      </div>
    </UCard>

    <!-- Row 2 : extra keys -->
    <UCard>
      <template #header>
        <div class="flex items-center justify-between">
          <h2 class="font-semibold">Дополнительные клавиши</h2>
          <UButton icon="i-lucide-plus" size="sm" @click="addExtra">
            Добавить
          </UButton>
        </div>
      </template>
      <div
        v-if="currentKeymap.extras.length === 0"
        class="text-sm text-(--ui-text-muted)"
      >
        Здесь можно добавить произвольные клавиши / бинды (медиа-клавиши,
        кнопки мыши и т.д.) для этого слоя.
      </div>
      <div v-else class="space-y-2">
        <div
          v-for="ex in currentKeymap.extras"
          :key="ex.id"
          class="grid grid-cols-[1fr_1fr_auto] gap-3 items-center p-2 rounded-md bg-(--ui-bg-muted)"
        >
          <UInput v-model="ex.name" placeholder="Название клавиши" />
          <UInput v-model="ex.action" placeholder="Действие" />
          <UButton
            icon="i-lucide-trash-2"
            color="error"
            variant="ghost"
            square
            aria-label="Удалить"
            @click="removeExtra(ex.id)"
          />
        </div>
      </div>
    </UCard>

    <KeyEditModal
      v-model="editOpen"
      :key-code="editKeyCode"
      :key-label="editKeyLabel"
      :action="editAction"
      @save="saveEdit"
      @clear="clearEdit"
    />

    <UModal v-model:open="newLayerOpen" title="Новый слой">
      <template #body>
        <UFormField label="Имя слоя">
          <UInput
            v-model="newLayerName"
            autofocus
            @keydown.enter="confirmNewLayer"
          />
        </UFormField>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton variant="ghost" color="neutral" @click="newLayerOpen = false">
            Отмена
          </UButton>
          <UButton @click="confirmNewLayer">Создать</UButton>
        </div>
      </template>
    </UModal>

    <UModal v-model:open="renameOpen" title="Переименовать слой">
      <template #body>
        <UFormField label="Имя слоя">
          <UInput
            v-model="renameDraft"
            autofocus
            @keydown.enter="confirmRename"
          />
        </UFormField>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton variant="ghost" color="neutral" @click="renameOpen = false">
            Отмена
          </UButton>
          <UButton @click="confirmRename">Сохранить</UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
