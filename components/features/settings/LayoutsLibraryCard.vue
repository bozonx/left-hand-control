<script setup lang="ts">
import type { LayoutLibraryEntry } from '~/composables/useLayoutLibrary'

defineProps<{
  entries: LayoutLibraryEntry[]
  currentLayoutId?: string
  isLayoutDirty: boolean
  applying: string
  applyError?: string | null
  libraryError?: string | null
  layoutsDir: string
}>()

defineEmits<{
  saveCurrent: []
  requestApplyEntry: [entry: LayoutLibraryEntry]
  requestApplyEmpty: []
  requestDelete: [entry: LayoutLibraryEntry]
}>()
</script>

<template>
  <UCard>
    <template #header>
      <div class="flex items-center justify-between gap-3 flex-wrap">
        <h2 class="font-semibold">{{ $t('settings.layoutsTitle') }}</h2>
        <UButton
          color="primary"
          icon="i-lucide-save"
          :disabled="!isLayoutDirty && !currentLayoutId"
          @click="$emit('saveCurrent')"
        >
          {{ $t('settings.saveCurrent') }}
        </UButton>
      </div>
    </template>

    <div class="space-y-3">
      <div
        v-if="isLayoutDirty"
        class="flex items-start gap-2 p-3 rounded border border-(--ui-warning)/40 bg-(--ui-warning)/10 text-sm"
      >
        <UIcon
          name="i-lucide-alert-triangle"
          class="text-(--ui-warning) mt-0.5 shrink-0"
        />
        <div>
          <div class="font-semibold">
            {{ $t('settings.dirtyBadgeTitle') }}
          </div>
          <div class="text-(--ui-text-muted)">
            {{ $t('settings.dirtyBadgeBody') }}
          </div>
        </div>
      </div>

      <p v-if="applyError" class="text-sm text-(--ui-error)">
        {{ applyError }}
      </p>

      <p v-if="libraryError" class="text-sm text-(--ui-error)">
        {{ libraryError }}
      </p>

      <ul class="divide-y divide-(--ui-border) border border-(--ui-border) rounded">
        <li
          v-for="entry in entries"
          :key="entry.id"
          class="flex items-center justify-between gap-3 p-3"
        >
          <div class="flex items-center gap-2 min-w-0">
            <UIcon
              :name="entry.builtin ? 'i-lucide-sparkles' : 'i-lucide-file'"
              :class="entry.builtin ? 'text-(--ui-primary)' : ''"
            />
            <div class="min-w-0">
              <div class="font-medium truncate flex items-center gap-2">
                {{ entry.name }}
                <UBadge
                  v-if="currentLayoutId === entry.id"
                  color="success"
                  variant="subtle"
                  size="sm"
                >
                  {{ $t('settings.activeBadge') }}
                </UBadge>
                <UBadge
                  v-if="entry.builtin"
                  color="primary"
                  variant="outline"
                  size="sm"
                >
                  {{ $t('settings.builtinBadge') }}
                </UBadge>
              </div>
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <UButton
              variant="outline"
              icon="i-lucide-rotate-ccw"
              :loading="applying === entry.id"
              :disabled="!!applying"
              @click="$emit('requestApplyEntry', entry)"
            >
              {{ $t('settings.applyBtn') }}
            </UButton>
            <UButton
              v-if="!entry.builtin"
              color="error"
              variant="ghost"
              icon="i-lucide-trash-2"
              :aria-label="$t('settings.deleteAria')"
              @click="$emit('requestDelete', entry)"
            />
          </div>
        </li>
      </ul>

      <div class="flex items-center justify-between gap-3 pt-2 border-t border-(--ui-border)">
        <div class="text-sm text-(--ui-text-muted)">
          {{ $t('settings.resetHint') }}
        </div>
        <UButton
          color="warning"
          variant="outline"
          icon="i-lucide-eraser"
          :loading="applying === 'empty'"
          :disabled="!!applying"
          @click="$emit('requestApplyEmpty')"
        >
          {{ $t('settings.resetBtn') }}
        </UButton>
      </div>

      <p class="text-xs text-(--ui-text-muted)">
        {{ $t('settings.userLayoutsDir') }}
        <code class="break-all">{{ layoutsDir || '…' }}</code>
      </p>
    </div>
  </UCard>
</template>
