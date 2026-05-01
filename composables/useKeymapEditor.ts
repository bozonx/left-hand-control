import { useKeymapLayerContext } from './useKeymapLayerContext'
import { useKeymapKeyEdit } from './useKeymapKeyEdit'
import { useKeymapExtras } from './useKeymapExtras'
import { useKeymapLayerOps } from './useKeymapLayerOps'

export function useKeymapEditor() {
  const layerCtx = useKeymapLayerContext()
  const keyEdit = useKeymapKeyEdit(layerCtx.currentKeymap, layerCtx.currentLayer)
  const extras = useKeymapExtras(layerCtx.currentKeymap, layerCtx.currentLayer)
  const layerOps = useKeymapLayerOps(layerCtx.selectedLayerId, layerCtx.currentLayer, layerCtx.config)

  return {
    config: layerCtx.config,
    selectedLayerId: layerCtx.selectedLayerId,
    keyLabelMode: layerCtx.keyLabelMode,
    layerItems: layerCtx.layerItems,
    currentLayer: layerCtx.currentLayer,
    currentKeymap: layerCtx.currentKeymap,
    editOpen: keyEdit.editOpen,
    editKeyCode: keyEdit.editKeyCode,
    editKeyLabel: computed(() => ''),
    editAction: keyEdit.editAction,
    openEdit: keyEdit.openEdit,
    saveEdit: keyEdit.saveEdit,
    clearEdit: keyEdit.clearEdit,
    swallowEdit: keyEdit.swallowEdit,
    addExtra: extras.addExtra,
    moveExtra: extras.moveExtra,
    removeExtra: extras.removeExtra,
    updateExtra: extras.updateExtra,
    renameOpen: layerOps.renameOpen,
    renameDraftName: layerOps.renameDraftName,
    renameDraftDescription: layerOps.renameDraftDescription,
    openRename: layerOps.openRename,
    confirmRename: layerOps.confirmRename,
    updateCurrentLayerDescription: layerOps.updateCurrentLayerDescription,
    affectedRulesCount: layerOps.affectedRulesCount,
    deleteConfirmOpen: layerOps.deleteConfirmOpen,
    requestDeleteSelectedLayer: layerOps.requestDeleteSelectedLayer,
    cancelDeleteSelectedLayer: layerOps.cancelDeleteSelectedLayer,
    deleteSelectedLayer: layerOps.deleteSelectedLayer,
    clearConfirmOpen: layerOps.clearConfirmOpen,
    requestClearSelectedLayer: layerOps.requestClearSelectedLayer,
    cancelClearSelectedLayer: layerOps.cancelClearSelectedLayer,
    clearSelectedLayer: layerOps.clearSelectedLayer,
    newLayerOpen: layerOps.newLayerOpen,
    newLayerName: layerOps.newLayerName,
    newLayerDescription: layerOps.newLayerDescription,
    openNewLayer: layerOps.openNewLayer,
    confirmNewLayer: layerOps.confirmNewLayer,
    cloneLayerOpen: layerOps.cloneLayerOpen,
    cloneDraftName: layerOps.cloneDraftName,
    openCloneLayer: layerOps.openCloneLayer,
    confirmCloneLayer: layerOps.confirmCloneLayer,
  }
}
