<script setup lang="ts">
defineProps<{
  error?: string | null
}>()

defineEmits<{
  retry: []
}>()
</script>

<template>
  <div class="min-h-screen flex items-center justify-center p-6 bg-(--ui-bg)">
    <UCard class="w-full max-w-lg">
      <div class="space-y-4 text-center">
        <div class="flex justify-center">
          <div
            class="w-10 h-10 rounded-full border-4 border-(--ui-border) border-t-(--ui-primary) animate-spin"
          />
        </div>

        <div class="space-y-1">
          <h1 class="text-xl font-semibold">{{ $t('app.title') }}</h1>
          <p class="text-sm text-(--ui-text-muted)">
            {{
              error
                ? $t('app.loadFailedBody')
                : $t('app.loading')
            }}
          </p>
        </div>

        <div
          v-if="error"
          class="rounded-md border border-(--ui-error)/40 bg-(--ui-error)/10 p-3 text-left text-sm text-(--ui-error) break-words"
        >
          {{ error }}
        </div>

        <div v-if="error" class="flex justify-center">
          <UButton
            color="primary"
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
