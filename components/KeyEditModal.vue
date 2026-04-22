<script setup lang="ts">
import FieldResetButton from '~/components/shared/FieldResetButton.vue'

const props = defineProps<{
  keyLabel: string
  keyCode: string
  action: string
}>()

const emit = defineEmits<{
  save: [action: string]
  clear: []
}>()

const open = defineModel<boolean>({ required: true })

const draft = ref(props.action)

watch(open, (v) => {
  if (v) draft.value = props.action
})

function save() {
  emit('save', draft.value.trim())
  open.value = false
}

function clear() {
  emit('clear')
  open.value = false
}
</script>

<template>
  <UModal
    v-model:open="open"
    :title="$t('keymap.editTitle', { label: keyLabel })"
    :ui="{ content: 'max-w-3xl' }"
  >
    <template #body>
      <div class="space-y-3">
        <div class="text-sm text-(--ui-text-muted)">
          <i18n-t keypath="keymap.keyCode" tag="span">
            <template #code>
              <code>{{ keyCode }}</code>
            </template>
          </i18n-t>
        </div>
        <ActionPickerBody v-model="draft" />
      </div>
    </template>
    <template #footer>
      <div class="flex justify-between w-full">
        <FieldResetButton
          v-if="action"
          :label="$t('common.clear')"
          @click="clear"
        />
        <div class="flex gap-2 ml-auto">
          <UButton color="neutral" variant="ghost" @click="open = false">
            {{ $t('common.cancel') }}
          </UButton>
          <UButton icon="i-lucide-check" @click="save">{{ $t('common.save') }}</UButton>
        </div>
      </div>
    </template>
  </UModal>
</template>
