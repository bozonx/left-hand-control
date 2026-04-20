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

const currentLayer = computed(() =>
  config.value.layers.find((l) => l.id === selectedLayerId.value),
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
    <UCard>
      <div class="flex flex-wrap items-end gap-3">
        <UFormField :label="$t('keymap.layerLabel')" class="flex-1 min-w-[220px]">
          <USelectMenu
            v-model="selectedLayerId"
            :items="layerItems"
            value-key="value"
            class="w-full"
          />
        </UFormField>
        <div class="flex gap-2">
          <UButton icon="i-lucide-plus" size="sm" @click="openNewLayer">
            {{ $t('keymap.newLayer') }}
          </UButton>
          <UButton
            icon="i-lucide-pencil"
            size="sm"
            color="neutral"
            variant="outline"
            :disabled="selectedLayerId === 'base'"
            @click="openRename"
          >
            {{ $t('keymap.edit') }}
          </UButton>
          <UButton
            icon="i-lucide-trash-2"
            size="sm"
            color="error"
            variant="outline"
            :disabled="selectedLayerId === 'base'"
            @click="deleteLayer"
          >
            {{ $t('keymap.delete') }}
          </UButton>
        </div>
      </div>
      <div
        v-if="currentLayer?.description"
        class="mt-3 text-sm text-(--ui-text-muted) p-3 rounded-md bg-(--ui-bg-muted) border border-(--ui-border)"
      >
        <UIcon name="i-lucide-info" class="w-3.5 h-3.5 mr-1 align-middle" />
        {{ currentLayer.description }}
      </div>
    </UCard>

    <!-- Row 1 : keyboard grid -->
    <UCard>
      <template #header>
        <h2 class="font-semibold">{{ $t('keymap.keyboardTitle') }}</h2>
      </template>
      <div class="grid grid-cols-1 xl:grid-cols-2 gap-6">
        <section>
          <div
            class="text-xs uppercase tracking-wide text-(--ui-text-muted) mb-2"
          >
            {{ $t('keymap.leftHand') }}
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
            {{ $t('keymap.rightHand') }}
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
          <div>
            <h2 class="font-semibold">{{ $t('keymap.extrasTitle') }}</h2>
            <p class="text-xs text-(--ui-text-muted) mt-1">
              {{ $t('keymap.extrasSub') }}
            </p>
          </div>
          <UButton icon="i-lucide-plus" size="sm" @click="addExtra">
            {{ $t('keymap.addExtra') }}
          </UButton>
        </div>
      </template>
      <div
        v-if="currentKeymap.extras.length === 0"
        class="text-sm text-(--ui-text-muted)"
      >
        {{ $t('keymap.extrasEmpty') }}
      </div>
      <div v-else class="space-y-2">
        <div
          v-for="ex in currentKeymap.extras"
          :key="ex.id"
          class="grid grid-cols-[1fr_1fr_auto] gap-3 items-start p-2 rounded-md bg-(--ui-bg-muted)"
        >
          <UFormField>
            <template #label>
              <FieldLabel
                :label="$t('keymap.extraKeyLabel')"
                :hint="$t('keymap.extraKeyHint')"
              />
            </template>
            <ActionPickerModal
              v-model="ex.name"
              key-only
              :placeholder="$t('rules.keyPh')"
            />
          </UFormField>
          <UFormField>
            <template #label>
              <FieldLabel
                :label="$t('keymap.extraActionLabel')"
                :hint="$t('keymap.extraActionHint')"
              />
            </template>
            <ActionPickerModal
              v-model="ex.action"
              allow-empty
              :placeholder="$t('rules.tapPh')"
            />
          </UFormField>
          <div class="pt-6">
            <UButton
              icon="i-lucide-trash-2"
              color="error"
              variant="ghost"
              square
              :aria-label="$t('keymap.deleteExtra')"
              @click="removeExtra(ex.id)"
            />
          </div>
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

    <UModal v-model:open="newLayerOpen" :title="$t('rules.newLayerTitle')">
      <template #body>
        <div class="space-y-3">
          <UFormField :label="$t('rules.layerName')">
            <UInput
              v-model="newLayerName"
              autofocus
              :placeholder="$t('rules.layerNamePh')"
              class="w-full"
              @keydown.enter="confirmNewLayer"
            />
          </UFormField>
          <UFormField :label="$t('rules.layerDesc')">
            <UTextarea
              v-model="newLayerDescription"
              :placeholder="$t('rules.layerDescPh')"
              class="w-full"
              :rows="2"
            />
          </UFormField>
        </div>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton variant="ghost" color="neutral" @click="newLayerOpen = false">
            {{ $t('common.cancel') }}
          </UButton>
          <UButton
            icon="i-lucide-check"
            :disabled="!newLayerName.trim()"
            @click="confirmNewLayer"
          >
            {{ $t('common.create') }}
          </UButton>
        </div>
      </template>
    </UModal>

    <UModal v-model:open="renameOpen" :title="$t('keymap.editLayerTitle')">
      <template #body>
        <div class="space-y-3">
          <UFormField :label="$t('rules.layerName')">
            <UInput
              v-model="renameDraftName"
              autofocus
              class="w-full"
              @keydown.enter="confirmRename"
            />
          </UFormField>
          <UFormField :label="$t('rules.layerDesc')">
            <UTextarea
              v-model="renameDraftDescription"
              class="w-full"
              :rows="2"
            />
          </UFormField>
        </div>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton variant="ghost" color="neutral" @click="renameOpen = false">
            {{ $t('common.cancel') }}
          </UButton>
          <UButton icon="i-lucide-check" @click="confirmRename">
            {{ $t('common.save') }}
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
