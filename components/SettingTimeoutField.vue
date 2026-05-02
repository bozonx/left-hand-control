<script setup lang="ts">
import type { ComponentPublicInstance } from 'vue'
import FieldLabel from '~/components/FieldLabel.vue'
import FieldResetButton from '~/components/shared/FieldResetButton.vue'

const props = withDefaults(defineProps<{
  label: string
  hint?: string
  defaultValue: number
  suffix?: string
  min?: number
  hintVisibleOn?: 'always' | 'group-hover' | 'group-hover-rule'
}>(), {
  suffix: '',
  min: 0,
  hintVisibleOn: 'always',
  hint: undefined,
})

const model = defineModel<number | undefined>()

const isEditing = ref(false)
const inputRef = ref<(ComponentPublicInstance & { $el: HTMLElement }) | null>(null)
const draft = ref('')

const isOverridden = computed(() => model.value !== undefined)
const displayValue = computed(() => model.value ?? props.defaultValue)

function startEdit() {
  if (isEditing.value) return
  draft.value = String(displayValue.value)
  isEditing.value = true
  nextTick(() => {
    const input = inputRef.value?.$el.querySelector('input')
    if (input) {
      input.focus()
      input.select()
    }
  })
}

function handleUpdate(val: string | number) {
  const next = String(val)
  draft.value = next
  if (next.trim() === '') return
  const parsed = Number(next)
  if (!Number.isFinite(parsed)) return
  model.value = Math.max(props.min, Math.round(parsed))
}

function reset() {
  model.value = undefined
  isEditing.value = false
}

function onBlur() {
  if (draft.value.trim() === '') {
    reset()
  } else {
    isEditing.value = false
  }
}
</script>

<template>
  <div
    class="group flex items-center justify-between py-1.5 px-2 -mx-2 rounded-lg transition-all duration-200 hover:bg-(--ui-bg-muted) cursor-pointer select-none"
    @click="startEdit"
  >
    <FieldLabel :label="label" :hint="hint" :hint-visible-on="hintVisibleOn" />

    <div class="flex items-center gap-2 overflow-hidden">
      <template v-if="!isEditing">
        <div class="flex items-center gap-1.5 min-w-0">
          <span
            class="text-sm font-mono truncate"
            :class="isOverridden ? 'text-(--ui-primary) font-medium' : 'text-(--ui-text-muted)'"
          >
            {{ displayValue }}<span v-if="suffix" class="ml-0.5 text-[10px] font-sans opacity-60 uppercase">{{ suffix }}</span>
          </span>
          <div v-if="isOverridden" @mousedown.stop @click.stop>
            <FieldResetButton
              :label="$t('common.reset')"
              @click="reset"
            />
          </div>
          <UIcon
            v-else
            name="i-lucide-pencil"
            class="w-3.5 h-3.5 text-(--ui-text-muted) opacity-0 group-hover:opacity-100 transition-opacity"
          />
        </div>
      </template>

      <div v-else class="flex items-center gap-1">
        <UInput
          ref="inputRef"
          :model-value="draft"
          type="number"
          size="xs"
          class="w-20 font-mono"
          :min="min"
          autofocus
          @update:model-value="handleUpdate"
          @blur="onBlur"
          @keydown.enter="isEditing = false"
          @keydown.esc="isEditing = false"
          @click.stop
        />
      </div>
    </div>
  </div>
</template>
