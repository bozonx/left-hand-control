<script setup lang="ts">
const appName = 'Left Hand Control'
const count = ref(0)

async function greet() {
  // Example Tauri command invocation (safe no-op in browser)
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const msg = await invoke<string>('greet', { name: 'Tauri' })
    // eslint-disable-next-line no-console
    console.log(msg)
  } catch (e) {
    // eslint-disable-next-line no-console
    console.warn('Tauri API not available (running in browser?)', e)
  }
}
</script>

<template>
  <UApp>
    <div class="min-h-screen flex items-center justify-center p-8">
      <UCard class="max-w-xl w-full">
        <template #header>
          <div class="flex items-center justify-between">
            <h1 class="text-xl font-semibold">{{ appName }}</h1>
            <UBadge color="primary" variant="subtle">Tauri + Nuxt</UBadge>
          </div>
        </template>

        <div class="space-y-4">
          <p class="text-sm text-(--ui-text-muted)">
            Desktop scaffold built with Nuxt 3, Nuxt UI v3, Tailwind v4 and Tauri 2.
          </p>

          <div class="flex items-center gap-3">
            <UButton icon="i-lucide-plus" @click="count++">
              Clicked {{ count }}
            </UButton>
            <UButton color="neutral" variant="outline" icon="i-lucide-terminal" @click="greet">
              Invoke Tauri
            </UButton>
          </div>
        </div>
      </UCard>
    </div>
  </UApp>
</template>
