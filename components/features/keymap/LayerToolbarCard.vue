<script setup lang="ts">
defineProps<{
  selectedLayerId: string
  layerItems: Array<{ label: string, value: string }>
  currentLayerDescription?: string
}>()

defineEmits<{
  'update:selectedLayerId': [value: string]
  create: []
  rename: []
  delete: []
}>()
</script>

<template>
  <UCard>
    <div class="flex flex-wrap items-end gap-3">
      <UFormField :label="$t('keymap.layerLabel')" class="flex-1 min-w-[220px]">
        <USelectMenu
          :model-value="selectedLayerId"
          :items="layerItems"
          value-key="value"
          class="w-full"
          @update:model-value="(value: string) => $emit('update:selectedLayerId', value)"
        />
      </UFormField>
      <div class="flex gap-2">
        <UButton icon="i-lucide-plus" size="sm" @click="$emit('create')">
          {{ $t('keymap.newLayer') }}
        </UButton>
        <UButton
          icon="i-lucide-pencil"
          size="sm"
          color="neutral"
          variant="outline"
          :disabled="selectedLayerId === 'base'"
          @click="$emit('rename')"
        >
          {{ $t('keymap.edit') }}
        </UButton>
        <UButton
          icon="i-lucide-trash-2"
          size="sm"
          color="error"
          variant="outline"
          :disabled="selectedLayerId === 'base'"
          @click="$emit('delete')"
        >
          {{ $t('keymap.delete') }}
        </UButton>
      </div>
    </div>
    <div
      v-if="currentLayerDescription"
      class="mt-3 text-sm text-(--ui-text-muted) p-3 rounded-md bg-(--ui-bg-muted) border border-(--ui-border)"
    >
      <UIcon name="i-lucide-info" class="w-3.5 h-3.5 mr-1 align-middle" />
      {{ currentLayerDescription }}
    </div>
  </UCard>
</template>
