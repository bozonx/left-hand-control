<script setup lang="ts">
const props = defineProps<{
  defaultValue: number
  suffix?: string
  min?: number
  inputClass?: string
}>()

const model = defineModel<number | undefined>({ default: undefined })

const isOverridden = computed(() => model.value !== undefined)

function startEdit() {
  model.value = props.defaultValue
}

function reset() {
  model.value = undefined
}
</script>

<template>
  <div class="flex items-center gap-2 min-h-[32px]">
    <template v-if="!isOverridden">
      <span class="text-sm text-(--ui-text-muted) font-mono">
        {{ defaultValue }}<span v-if="suffix"> {{ suffix }}</span>
      </span>
      <UButton
        icon="i-lucide-pencil"
        size="xs"
        variant="ghost"
        color="neutral"
        square
        :aria-label="$t('common.override')"
        @click="startEdit"
      />
    </template>
    <template v-else>
      <UInput
        v-model.number="model"
        type="number"
        :min="min ?? 0"
        :class="inputClass ?? 'w-28'"
      />
      <span v-if="suffix" class="text-xs text-(--ui-text-muted)">{{ suffix }}</span>
      <UButton
        icon="i-lucide-rotate-ccw"
        size="xs"
        variant="ghost"
        color="neutral"
        square
        :aria-label="$t('common.reset')"
        @click="reset"
      />
    </template>
  </div>
</template>
