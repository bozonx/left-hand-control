<script setup lang="ts">
import FieldResetButton from '~/components/shared/FieldResetButton.vue'
import { parseTextAction } from '~/types/config'
import { isCanonicalAction, normalizeActionValue } from '~/utils/actionSyntax'

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
const containerRef = ref<HTMLElement | null>(null)
const normalizedDraft = computed(() => {
  const raw = draft.value ?? ''
  return parseTextAction(raw) !== null ? raw : raw.trim()
})
const draftInvalid = computed(() => !isCanonicalAction(normalizedDraft.value))

watch(open, (v) => {
  if (v) {
    draft.value = typeof props.action === 'string' ? props.action : ''
    nextTick(() => containerRef.value?.focus())
  }
})

function save() {
  const next = normalizeActionValue(draft.value)
  if (next === null) return
  emit('save', next)
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
  const next = normalizeActionValue(value)
  if (next === null) return
  emit('save', next)
  open.value = false
}
</script>

<template>
  <UModal v-model:open="open" fullscreen :ui="{ body: 'overflow-y-hidden' }">
    <template #header>
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
        <UButton icon="i-lucide-check" :disabled="draftInvalid" @click="save">
          {{ $t('common.save') }}
        </UButton>
      </div>
    </template>

    <template #body>
      <div
        ref="containerRef"
        tabindex="-1"
        class="mx-auto flex h-full w-full max-w-7xl flex-col"
        data-testid="key-edit-view"
      >
        <ActionPickerBody v-model="draft" spacious @pick="pickAndSave" />
        <p
          v-if="draftInvalid"
          class="mt-3 text-sm text-(--ui-error)"
        >
          {{ $t('picker.invalidValue') }}
        </p>
      </div>
    </template>
  </UModal>
</template>
