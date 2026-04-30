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
  swallowEdit,
  addExtra,
  moveExtra,
  removeExtra,
  updateExtra,
  renameOpen,
  renameDraftName,
  renameDraftDescription,
  openRename,
  confirmRename,
  updateCurrentLayerDescription,
  affectedRulesCount,
  deleteConfirmOpen,
  requestDeleteSelectedLayer,
  cancelDeleteSelectedLayer,
  deleteSelectedLayer,
  clearConfirmOpen,
  requestClearSelectedLayer,
  cancelClearSelectedLayer,
  clearSelectedLayer,
  newLayerOpen,
  newLayerName,
  newLayerDescription,
  openNewLayer,
  confirmNewLayer,
  cloneLayerOpen,
  cloneDraftName,
  openCloneLayer,
  confirmCloneLayer,
} = useKeymapEditor()

const extraIds = computed(() => currentKeymap.value?.extras.map((e) => e.id) ?? [])
const { selectedId: selectedExtraId, select: selectExtra, containerRef: extrasContainerRef } = useListKeyboardNavigation({
  ids: extraIds,
  move: (id: string, delta: number) => moveExtra(id, delta < 0 ? 'up' : 'down'),
})
</script>

<template>
  <div class="space-y-4">
    <LayerToolbarCard
      v-model:selected-layer-id="selectedLayerId"
      :layer-items="layerItems"
      :current-layer-name="currentLayer?.name"
      :current-layer-description="currentLayer?.description"
      @create="openNewLayer"
      @clone="openCloneLayer"
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
        @clear="requestClearSelectedLayer"
      />

      <div ref="extrasContainerRef">
        <ExtrasCard
          :extras="currentKeymap.extras"
          :selected-id="selectedExtraId"
          @select="selectExtra"
          @add="addExtra"
          @update-extra="updateExtra"
          @move-up="(id) => moveExtra(id, 'up')"
          @move-down="(id) => moveExtra(id, 'down')"
          @remove="removeExtra"
        />
      </div>
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
      @swallow="swallowEdit"
    />

    <LayerEditorModal
      v-model="newLayerOpen"
      v-model:name="newLayerName"
      v-model:description="newLayerDescription"
      :title="$t('rules.newLayerTitle')"
      :confirm-label="$t('common.create')"
      :name-placeholder="$t('rules.layerNamePh')"
      @confirm="confirmNewLayer"
    />

    <LayerEditorModal
      v-model="renameOpen"
      v-model:name="renameDraftName"
      v-model:description="renameDraftDescription"
      :title="$t('keymap.renameLayerTitle')"
      :confirm-label="$t('common.save')"
      @confirm="confirmRename"
    />

    <LayerEditorModal
      v-model="cloneLayerOpen"
      v-model:name="cloneDraftName"
      :title="$t('keymap.cloneLayerTitle')"
      :confirm-label="$t('common.duplicate')"
      :name-placeholder="$t('rules.layerNamePh')"
      @confirm="confirmCloneLayer"
    />

    <UModal
      v-model:open="deleteConfirmOpen"
      :title="$t('keymap.deleteLayerTitle', { name: currentLayer?.name ?? '' })"
    >
      <template #body>
        <p class="text-sm">
          {{ $t('keymap.deleteLayerBody') }}
        </p>
        <p
          v-if="affectedRulesCount === 1"
          class="mt-2 text-sm text-(--ui-warning)"
        >
          {{ $t('keymap.deleteLayerRulesHint_one') }}
        </p>
        <p
          v-else-if="affectedRulesCount > 1"
          class="mt-2 text-sm text-(--ui-warning)"
        >
          {{ $t('keymap.deleteLayerRulesHint_other', { count: affectedRulesCount }) }}
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

    <UModal
      v-model:open="clearConfirmOpen"
      :title="$t('keymap.clearLayerTitle')"
    >
      <template #body>
        <p class="text-sm">
          {{ $t('keymap.clearLayerBody') }}
        </p>
      </template>
      <template #footer>
        <div class="flex gap-2 justify-end w-full">
          <UButton
            variant="ghost"
            color="neutral"
            @click="cancelClearSelectedLayer"
          >
            {{ $t('common.cancel') }}
          </UButton>
          <UButton
            color="error"
            icon="i-lucide-eraser"
            @click="clearSelectedLayer"
          >
            {{ $t('common.clear') }}
          </UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
