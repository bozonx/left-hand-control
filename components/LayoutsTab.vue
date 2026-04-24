<script setup lang="ts">
import LayoutsLibraryCard from "~/components/features/settings/LayoutsLibraryCard.vue";

const {
  currentLayoutId,
  currentLayoutDescription,
  isLayoutDirty,
  library,
  applying,
  applyError,
  pendingApply,
  requestApplyEntry,
  cancelApply,
  confirmApply,
  createFromEmpty,
  createFromIvanK,
  saveModalOpen,
  saveName,
  saveBusy,
  saveError,
  openSaveModal,
  openSaveAsModal,
  performSave,
  closeSaveModal,
  editModalOpen,
  editName,
  editDescription,
  editBusy,
  editError,
  editPending,
  openEditModal,
  performEdit,
  closeEditModal,
  overwriteConfirmOpen,
  overwriteTargetName,
  confirmOverwrite,
  closeOverwriteConfirm,
  resetConfirmOpen,
  resetBusy,
  requestReset,
  confirmReset,
  closeResetConfirm,
  deletePending,
  deleteBusy,
  confirmDelete,
  clearDeletePending,
} = useSettingsScreen();
</script>

<template>
  <div class="mx-auto w-full max-w-5xl space-y-4">
    <LayoutsLibraryCard
      :entries="library.entries.value"
      :current-layout-id="currentLayoutId"
      :current-layout-description="currentLayoutDescription"
      :is-layout-dirty="isLayoutDirty"
      :applying="applying"
      :apply-error="applyError"
      :library-error="library.error.value"
      :layouts-dir="library.layoutsDir.value"
      @save-current="openSaveModal"
      @save-as="openSaveAsModal"
      @request-apply-entry="requestApplyEntry"
      @create-from-empty="createFromEmpty"
      @create-from-ivan-k="createFromIvanK"
      @request-edit="openEditModal"
      @request-reset="requestReset"
      @request-delete="(entry) => (deletePending = entry)"
    />

  </div>
</template>

