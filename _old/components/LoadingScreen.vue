<script setup lang="ts">
defineProps<{
  error?: string | null
}>()

defineEmits<{
  retry: []
}>()
</script>

<template>
  <div class="h-screen flex items-center justify-center p-8 bg-(--ui-bg)">
    <UCard class="w-full max-w-md">
      <div class="space-y-4 text-center">
        <div class="flex justify-center">
          <div
            class="w-8 h-8 rounded-full border-[3px] border-(--ui-border) border-t-(--ui-primary) animate-spin"
          />
        </div>

        <div class="space-y-1">
          <h1 class="text-base font-semibold">{{ $t('app.title') }}</h1>
          <p class="text-xs text-(--ui-text-muted)">
            {{
              error
                ? $t('app.loadFailedBody')
                : $t('app.loading')
            }}
          </p>
        </div>

        <div
          v-if="error"
          class="rounded-md border border-(--ui-error)/40 bg-(--ui-error)/10 p-3 text-left text-xs text-(--ui-error) break-words"
        >
          {{ error }}
        </div>

        <div v-if="error" class="flex justify-center">
          <UButton
            color="primary"
            size="sm"
            icon="i-lucide-refresh-cw"
            @click="$emit('retry')"
          >
            {{ $t('common.retry') }}
          </UButton>
        </div>
      </div>
    </UCard>
  </div>
</template>
