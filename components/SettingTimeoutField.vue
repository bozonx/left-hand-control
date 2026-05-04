<script setup lang="ts">
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
const inputRef = ref<{ focus: () => void, select: () => void } | null>(null)
const draft = ref('')
let isJustFocused = false

const isOverridden = computed(() => model.value !== undefined)
const displayValue = computed(() => model.value ?? props.defaultValue)

function startEdit() {
  if (isEditing.value) return
  draft.value = String(displayValue.value)
  isEditing.value = true
  isJustFocused = true
  nextTick(() => {
    inputRef.value?.focus()
    inputRef.value?.select()
    setTimeout(() => {
      isJustFocused = false
    }, 150)
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
  if (isJustFocused) return
  if (draft.value.trim() === '') {
    reset()
  } else {
    isEditing.value = false
  }
}
</script>

<template>
  <div
    v-if="!isEditing"
    class="group -mx-2 flex items-center justify-between rounded-lg px-2 py-1.5 transition-all duration-200 hover:bg-(--ui-bg-muted)"
    @click.stop="startEdit"
  >
    <button
      type="button"
      class="flex min-w-0 flex-1 items-center justify-between gap-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-(--ui-primary)"
      @click.stop="startEdit"
    >
      <FieldLabel :label="label" :hint="hint" :hint-visible-on="hintVisibleOn" />

      <div class="flex min-w-0 items-center gap-1.5 overflow-hidden">
        <span
          class="truncate font-mono text-sm"
          :class="isOverridden ? 'text-(--ui-primary) font-medium' : 'text-(--ui-text-muted)'"
        >
          {{ displayValue }}<span v-if="suffix" class="ml-0.5 font-sans text-[10px] uppercase opacity-60">{{ suffix }}</span>
        </span>
        <UIcon
          v-if="!isOverridden"
          name="i-lucide-pencil"
          class="h-3.5 w-3.5 text-(--ui-text-muted) opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
        />
      </div>
    </button>
    <div v-if="isOverridden" class="ml-2 shrink-0" @mousedown.stop @click.stop>
      <FieldResetButton
        :label="$t('common.reset')"
        @click="reset"
      />
    </div>
  </div>

  <div v-else class="-mx-2 flex items-center justify-between rounded-lg px-2 py-1.5">
    <FieldLabel :label="label" :hint="hint" :hint-visible-on="hintVisibleOn" />
    <NumericInput
      ref="inputRef"
      :model-value="draft"
      size="xs"
      class="w-20 font-mono"
      :min="min"
      @update:model-value="handleUpdate"
      @blur="onBlur"
      @keydown.enter="isEditing = false"
      @keydown.esc="isEditing = false"
      @click.stop
    />
  </div>
</template>
