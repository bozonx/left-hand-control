<script setup lang="ts">
const props = defineProps<{
  title: string
  confirmLabel: string
  namePlaceholder?: string
}>()

const open = defineModel<boolean>({ required: true })
const name = defineModel<string>('name', { required: true })

defineEmits<{
  confirm: []
}>()
</script>

<template>
  <UModal v-model:open="open" :title="title">
    <template #body>
      <UFormField :label="$t('rules.layerName')">
        <UInput
          v-model="name"
          autofocus
          :placeholder="namePlaceholder"
          class="w-full"
          @keydown.enter="$emit('confirm')"
        />
      </UFormField>
    </template>
    <template #footer>
      <div class="flex gap-2 justify-end w-full">
        <UButton
          variant="ghost"
          color="neutral"
          @click="open = false"
        >
          {{ $t('common.cancel') }}
        </UButton>
        <UButton
          icon="i-lucide-check"
          :disabled="!name.trim()"
          @click="$emit('confirm')"
        >
          {{ confirmLabel }}
        </UButton>
      </div>
    </template>
  </UModal>
</template>
