<script setup lang="ts">
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
  <UModal v-model:open="open" :title="`Редактирование: ${keyLabel}`">
    <template #body>
      <div class="space-y-3">
        <div class="text-sm text-(--ui-text-muted)">
          Код клавиши: <code>{{ keyCode }}</code>
        </div>
        <UFormField label="Действие">
          <ActionPicker
            v-model="draft"
            allow-empty
            placeholder="например: Ctrl+C, BrowserBack"
          />
        </UFormField>
        <p class="text-xs text-(--ui-text-muted)">
          Выберите клавишу из списка или введите произвольную строку и
          нажмите Enter — она будет использована как есть.
        </p>
      </div>
    </template>
    <template #footer>
      <div class="flex justify-between w-full">
        <UButton
          v-if="action"
          color="error"
          variant="ghost"
          icon="i-lucide-trash-2"
          @click="clear"
        >
          Очистить
        </UButton>
        <div class="flex gap-2 ml-auto">
          <UButton color="neutral" variant="ghost" @click="open = false">
            Отмена
          </UButton>
          <UButton icon="i-lucide-check" @click="save">Сохранить</UButton>
        </div>
      </div>
    </template>
  </UModal>
</template>
