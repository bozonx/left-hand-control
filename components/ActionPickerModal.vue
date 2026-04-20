<script setup lang="ts">
const props = defineProps<{
  allowEmpty?: boolean
  placeholder?: string
  keyOnly?: boolean
  title?: string
}>()

const model = defineModel<string>({ default: '' })

const { displayAction } = useMacros()

const open = ref(false)
const draft = ref('')

const displayLabel = computed(() => displayAction(model.value) || model.value)

function openModal() {
  draft.value = model.value
  open.value = true
}

function apply() {
  model.value = (draft.value ?? '').trim()
  open.value = false
}

function clear() {
  model.value = ''
  open.value = false
}
</script>

<template>
  <div class="flex items-center gap-1 w-full">
    <button
      type="button"
      class="flex-1 min-w-0 h-8 px-2.5 flex items-center gap-2 rounded-md border border-(--ui-border) bg-(--ui-bg) hover:bg-(--ui-bg-elevated) text-left text-sm transition-colors"
      @click="openModal"
    >
      <UIcon
        :name="model ? 'i-lucide-square-mouse-pointer' : 'i-lucide-plus'"
        class="shrink-0 w-4 h-4 text-(--ui-text-muted)"
      />
      <span v-if="displayLabel" class="truncate" :title="model">
        {{ displayLabel }}
      </span>
      <span v-else class="text-(--ui-text-muted) truncate">
        {{ placeholder ?? $t('picker.chooseAction') }}
      </span>
    </button>
    <UButton
      v-if="allowEmpty && model"
      icon="i-lucide-x"
      size="xs"
      color="neutral"
      variant="ghost"
      square
      :aria-label="$t('picker.clearAria')"
      @click="model = ''"
    />
  </div>

  <UModal
    v-model:open="open"
    :title="title ?? (keyOnly ? $t('picker.titleKey') : $t('picker.titleAction'))"
    :ui="{ content: 'max-w-3xl' }"
  >
    <template #body>
      <ActionPickerBody v-model="draft" :key-only="keyOnly" />
    </template>
    <template #footer>
      <div class="flex justify-between w-full gap-2">
        <UButton
          v-if="allowEmpty && model"
          color="error"
          variant="ghost"
          icon="i-lucide-trash-2"
          @click="clear"
        >
          {{ $t('common.clear') }}
        </UButton>
        <div class="flex gap-2 ml-auto">
          <UButton color="neutral" variant="ghost" @click="open = false">
            {{ $t('common.cancel') }}
          </UButton>
          <UButton icon="i-lucide-check" @click="apply">{{ $t('common.apply') }}</UButton>
        </div>
      </div>
    </template>
  </UModal>
</template>
