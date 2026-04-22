<script setup lang="ts">
import type { SystemMacro } from '~/utils/systemMacros'

defineProps<{
  systemOpen: boolean
  usage: Record<string, string[]>
  systemMacros: SystemMacro[]
}>()

defineEmits<{
  'update:systemOpen': [value: boolean]
  clone: [macro: SystemMacro]
}>()

function stepsPreview(sys: SystemMacro): string {
  return sys.steps.map((step) => step.keystroke).join(' → ')
}
</script>

<template>
  <UCard>
    <template #header>
      <button
        type="button"
        class="flex items-center justify-between gap-3 w-full text-left"
        :aria-expanded="systemOpen"
        @click="$emit('update:systemOpen', !systemOpen)"
      >
        <div>
          <h2 class="text-sm font-semibold flex items-center gap-2">
            <UIcon
              :name="systemOpen ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'"
              class="text-(--ui-text-muted)"
            />
            {{ $t('macros.systemTitle') }}
            <UBadge color="neutral" variant="subtle" size="sm">
              {{ systemMacros.length }}
            </UBadge>
          </h2>
          <p class="text-xs text-(--ui-text-muted) mt-0.5">
            {{ $t('macros.systemSub') }}
          </p>
        </div>
      </button>
    </template>

    <div v-show="systemOpen">
      <div
        v-if="systemMacros.length === 0"
        class="text-sm text-(--ui-text-muted)"
      >
        {{ $t('macros.systemEmpty') }}
      </div>

      <div v-else class="overflow-x-auto">
        <table class="w-full text-sm">
          <thead>
            <tr
              class="text-left text-xs text-(--ui-text-muted) border-b border-(--ui-border)"
            >
              <th class="py-2 pr-3 font-medium">{{ $t('macros.colId') }}</th>
              <th class="py-2 pr-3 font-medium">{{ $t('macros.colName') }}</th>
              <th class="py-2 pr-3 font-medium">{{ $t('macros.colSteps') }}</th>
              <th class="py-2 pr-3 font-medium w-px"></th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="sys in systemMacros"
              :key="sys.id"
              class="border-b border-(--ui-border) last:border-b-0 align-top"
            >
              <td class="py-2 pr-3 font-mono text-xs whitespace-nowrap">
                {{ sys.id }}
              </td>
              <td class="py-2 pr-3">
                <div>{{ sys.name }}</div>
                <div
                  v-if="sys.description"
                  class="text-xs text-(--ui-text-muted) mt-0.5"
                >
                  {{ sys.description }}
                </div>
              </td>
              <td class="py-2 pr-3">
                <code class="text-xs font-mono text-(--ui-text-muted)">
                  {{ stepsPreview(sys) }}
                </code>
                <div
                  v-if="usage[sys.id] && usage[sys.id].length"
                  class="flex flex-wrap gap-1 mt-1"
                >
                  <UBadge
                    v-for="place in usage[sys.id]"
                    :key="place"
                    color="neutral"
                    variant="subtle"
                    size="sm"
                    class="font-mono"
                  >
                    {{ place }}
                  </UBadge>
                </div>
              </td>
              <td class="py-2 pr-3 whitespace-nowrap">
                <UButton
                  size="xs"
                  variant="outline"
                  icon="i-lucide-copy-plus"
                  @click="$emit('clone', sys)"
                >
                  {{ $t('macros.cloneBtn') }}
                </UButton>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </UCard>
</template>
