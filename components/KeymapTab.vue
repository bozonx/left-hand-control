<script setup lang="ts">
import LayerToolbarCard from '~/components/features/keymap/LayerToolbarCard.vue'
import KeyboardGridCard from '~/components/features/keymap/KeyboardGridCard.vue'
import ExtrasCard from '~/components/features/keymap/ExtrasCard.vue'
import LayerEditorModal from '~/components/shared/LayerEditorModal.vue'
import { keyLabel } from '~/utils/keys'
const {
  selectedLayerId,
  keyLabelMode,
  layerItems,
  currentLayer,
  currentKeymap,
  editOpen,
  editKeyCode,
  editAction,
  openEdit,
  saveEdit,
  clearEdit,
  addExtra,
  moveExtra,
  removeExtra,
  renameOpen,
  renameDraftName,
  openRename,
  confirmRename,
  updateCurrentLayerDescription,
  deleteConfirmOpen,
  requestDeleteSelectedLayer,
  cancelDeleteSelectedLayer,
  deleteSelectedLayer,
  newLayerOpen,
  newLayerName,
  openNewLayer,
  confirmNewLayer,
} = useKeymapEditor()
</script>

<template>
  <div class="space-y-4">
    <LayerToolbarCard
      v-model:selected-layer-id="selectedLayerId"
      :layer-items="layerItems"
      :current-layer-name="currentLayer?.name"
      :current-layer-description="currentLayer?.description"
      @create="openNewLayer"
      @rename="openRename"
      @update-description="updateCurrentLayerDescription"
      @delete="requestDeleteSelectedLayer"
    />

    <template v-if="currentLayer">
      <KeyboardGridCard
        :current-keymap="currentKeymap"
        :key-label-mode="keyLabelMode"
        @update:key-label-mode="(value) => { keyLabelMode = value }"
        @edit="openEdit"
      />

      <ExtrasCard
        :extras="currentKeymap.extras"
        @add="addExtra"
        @move-up="(id) => moveExtra(id, 'up')"
        @move-down="(id) => moveExtra(id, 'down')"
        @remove="removeExtra"
      />
    </template>
    <UCard v-else>
      <div class="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
        <div>
          <h2 class="text-sm font-semibold">{{ $t('keymap.emptyTitle') }}</h2>
          <p class="mt-1 text-sm text-(--ui-text-muted)">
            {{ $t('keymap.emptyBody') }}
          </p>
        </div>
        <UButton icon="i-lucide-plus" size="sm" @click="openNewLayer">
          {{ $t('keymap.newLayer') }}
        </UButton>
      </div>
    </UCard>

    <KeyEditModal
      v-model="editOpen"
      :key-code="editKeyCode"
      :key-label="keyLabel(editKeyCode, keyLabelMode)"
      :action="editAction"
      @save="saveEdit"
      @clear="clearEdit"
    />

    <LayerEditorModal
      v-model="newLayerOpen"
      v-model:name="newLayerName"
      :title="$t('rules.newLayerTitle')"
      :confirm-label="$t('common.create')"
      :name-placeholder="$t('rules.layerNamePh')"
      @confirm="confirmNewLayer"
    />

    <LayerEditorModal
      v-model="renameOpen"
      v-model:name="renameDraftName"
      :title="$t('keymap.editLayerTitle')"
      :confirm-label="$t('common.save')"
      @confirm="confirmRename"
    />

    <UModal
      v-model:open="deleteConfirmOpen"
      :title="$t('keymap.deleteLayerTitle', { name: currentLayer?.name ?? '' })"
    >
      <template #body>
        <p class="text-sm">
          {{ $t('keymap.deleteLayerBody') }}
        </p>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton
            variant="ghost"
            color="neutral"
            @click="cancelDeleteSelectedLayer"
          >
            {{ $t('common.cancel') }}
          </UButton>
          <UButton
            color="error"
            icon="i-lucide-trash-2"
            @click="deleteSelectedLayer"
          >
            {{ $t('common.delete') }}
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
