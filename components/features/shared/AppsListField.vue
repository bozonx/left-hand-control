<script setup lang="ts">
import { ref } from 'vue'

// Reusable editor for a list of substrings matched (case-insensitive,
// OR) against the focused window's title and app id. Empty list means
// "do not check apps".

const props = defineProps<{
  modelValue: string[]
  label?: string
  hint?: string
  placeholder?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string[]]
}>()

const draft = ref('')

function add() {
  const value = draft.value.trim()
  if (!value) return
  if (props.modelValue.includes(value)) {
    draft.value = ''
    return
  }
  emit('update:modelValue', [...props.modelValue, value])
  draft.value = ''
}

function remove(index: number) {
  const next = [...props.modelValue]
  next.splice(index, 1)
  emit('update:modelValue', next)
}

function onEnter(event: KeyboardEvent) {
  event.preventDefault()
  add()
}
</script>

<template>
  <UFormField :label="label" :help="hint">
    <div class="flex flex-col gap-2">
      <div class="flex gap-2">
        <UInput
          v-model="draft"
          class="flex-1"
          :placeholder="placeholder"
          @keydown.enter="onEnter"
        />
        <UButton
          color="neutral"
          variant="subtle"
          icon="i-lucide-plus"
          :disabled="!draft.trim()"
          @click="add"
        >
          {{ $t('common.add') }}
        </UButton>
      </div>
      <div v-if="modelValue.length > 0" class="flex flex-wrap gap-2">
        <UBadge
          v-for="(item, index) in modelValue"
          :key="`${item}-${index}`"
          color="neutral"
          variant="soft"
          class="flex items-center gap-1"
        >
          <span>{{ item }}</span>
          <UButton
            color="neutral"
            variant="link"
            size="xs"
            icon="i-lucide-x"
            :aria-label="$t('common.delete')"
            class="-mr-1"
            @click="remove(index)"
          />
        </UBadge>
      </div>
    </div>
  </UFormField>
</template>
