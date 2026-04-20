<script setup lang="ts">
import LayerToolbarCard from '~/components/features/keymap/LayerToolbarCard.vue'
import KeyboardGridCard from '~/components/features/keymap/KeyboardGridCard.vue'
import ExtrasCard from '~/components/features/keymap/ExtrasCard.vue'
import LayerEditorModal from '~/components/shared/LayerEditorModal.vue'
const {
  selectedLayerId,
  layerItems,
  currentLayer,
  currentKeymap,
  editOpen,
  editKeyCode,
  editKeyLabel,
  editAction,
  openEdit,
  saveEdit,
  clearEdit,
  addExtra,
  removeExtra,
  renameOpen,
  renameDraftName,
  renameDraftDescription,
  openRename,
  confirmRename,
  deleteSelectedLayer,
  newLayerOpen,
  newLayerName,
  newLayerDescription,
  openNewLayer,
  confirmNewLayer,
} = useKeymapEditor()
</script>

<template>
  <div class="space-y-4">
    <LayerToolbarCard
      v-model:selected-layer-id="selectedLayerId"
      :layer-items="layerItems"
      :current-layer-description="currentLayer?.description"
      @create="openNewLayer"
      @rename="openRename"
      @delete="deleteSelectedLayer"
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
