<script setup lang="ts">
import LayerToolbarCard from '~/components/features/keymap/LayerToolbarCard.vue'
import KeyboardGridCard from '~/components/features/keymap/KeyboardGridCard.vue'
import ExtrasCard from '~/components/features/keymap/ExtrasCard.vue'
import LayerEditorModal from '~/components/shared/LayerEditorModal.vue'
import { randomId } from '~/utils/keys'
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

const currentLayer = computed(() =>
  config.value.layers.find((l) => l.id === selectedLayerId.value),
)

function ensureLayerKeymap(id: string) {
  if (!config.value.layerKeymaps[id]) {
    config.value.layerKeymaps[id] = { keys: {}, extras: [] }
  }
  return config.value.layerKeymaps[id]
}

const currentKeymap = computed(() => {
  const id = selectedLayerId.value
  return ensureLayerKeymap(id)
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
const renameDraftName = ref('')
const renameDraftDescription = ref('')
function openRename() {
  const l = currentLayer.value
  renameDraftName.value = l?.name ?? ''
  renameDraftDescription.value = l?.description ?? ''
  renameOpen.value = true
}
function confirmRename() {
  const l = currentLayer.value
  if (l) {
    l.name = renameDraftName.value.trim() || l.name
    l.description = renameDraftDescription.value.trim() || undefined
  }
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
const newLayerDescription = ref('')
function openNewLayer() {
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
  selectedLayerId.value = id
  newLayerOpen.value = false
}
</script>

<template>
  <div class="space-y-4">
    <LayerToolbarCard
      v-model:selected-layer-id="selectedLayerId"
      :layer-items="layerItems"
      :current-layer-description="currentLayer?.description"
      @create="openNewLayer"
      @rename="openRename"
      @delete="deleteLayer"
    />

    <KeyboardGridCard
      :current-keymap="currentKeymap"
      @edit="openEdit"
    />

    <ExtrasCard
      :extras="currentKeymap.extras"
      @add="addExtra"
      @remove="removeExtra"
    />

    <KeyEditModal
      v-model="editOpen"
      :key-code="editKeyCode"
      :key-label="editKeyLabel"
      :action="editAction"
      @save="saveEdit"
      @clear="clearEdit"
    />

    <LayerEditorModal
      v-model="newLayerOpen"
      v-model:name="newLayerName"
      v-model:description="newLayerDescription"
      :title="$t('rules.newLayerTitle')"
      :confirm-label="$t('common.create')"
      :name-placeholder="$t('rules.layerNamePh')"
      :description-placeholder="$t('rules.layerDescPh')"
      @confirm="confirmNewLayer"
    />

    <LayerEditorModal
      v-model="renameOpen"
      v-model:name="renameDraftName"
      v-model:description="renameDraftDescription"
      :title="$t('keymap.editLayerTitle')"
      :confirm-label="$t('common.save')"
      @confirm="confirmRename"
    />
  </div>
</template>
