<script setup lang="ts">
import FieldResetButton from '~/components/shared/FieldResetButton.vue'

const props = defineProps<{
  keyLabel: string
  keyCode: string
  action?: string | null
}>()

const emit = defineEmits<{
  save: [action: string]
  clear: []
  swallow: []
}>()

const open = defineModel<boolean>({ required: true })

const draft = ref(typeof props.action === 'string' ? props.action : '')
const pickerRef = ref<HTMLElement | null>(null)

watch(open, (v) => {
  if (v) {
    draft.value = typeof props.action === 'string' ? props.action : ''
    nextTick(() => pickerRef.value?.focus())
  }
})

function save() {
  emit('save', draft.value.trim())
  open.value = false
}

function clear() {
  emit('clear')
  open.value = false
}

function swallow() {
  emit('swallow')
  open.value = false
}

function pickAndSave(value: string) {
  draft.value = value
  emit('save', value.trim())
  open.value = false
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      ref="pickerRef"
      class="fixed inset-0 z-50 flex flex-col bg-(--ui-bg) text-(--ui-text) outline-none"
      role="dialog"
      aria-modal="true"
      :aria-label="$t('keymap.editTitle', { label: keyLabel })"
      tabindex="-1"
      data-testid="key-edit-view"
      @keydown.esc="open = false"
    >
      <header class="shrink-0 border-b border-(--ui-border) bg-(--ui-bg-elevated)/80 px-4 py-3">
        <div class="mx-auto flex w-full max-w-7xl items-center gap-3">
          <UButton
            icon="i-lucide-arrow-left"
            color="neutral"
            variant="ghost"
            :aria-label="$t('common.cancel')"
            @click="open = false"
          />
          <div class="min-w-0 flex-1">
            <h2 class="truncate text-base font-semibold">
              {{ $t('keymap.editTitle', { label: keyLabel }) }}
            </h2>
            <div class="truncate text-xs text-(--ui-text-muted)">
              <i18n-t keypath="keymap.keyCode" tag="span">
                <template #code>
                  <code>{{ keyCode }}</code>
                </template>
              </i18n-t>
            </div>
          </div>
          <FieldResetButton
            v-if="action !== undefined"
            :label="$t('common.clear')"
            @click="clear"
          />
          <UButton color="neutral" variant="ghost" @click="swallow">
            {{ $t('keymap.swallowAction') }}
          </UButton>
          <UButton color="neutral" variant="ghost" @click="open = false">
            {{ $t('common.cancel') }}
          </UButton>
          <UButton icon="i-lucide-check" @click="save">{{ $t('common.save') }}</UButton>
        </div>
      </header>

      <main class="min-h-0 flex-1 overflow-hidden px-4 py-4">
        <div class="mx-auto flex h-full w-full max-w-7xl flex-col">
          <ActionPickerBody v-model="draft" spacious @pick="pickAndSave" />
        </div>
      </main>
    </div>
  </Teleport>
</template>
