<script setup lang="ts">
import LandingTheme from 'vitepress-theme-neptu-landing'
import {
  ColorThemePicker,
  StylePresetPicker,
} from 'vitepress-theme-neptu-landing/components'

const { Layout: LandingLayout } = LandingTheme
</script>

<template>
  <LandingLayout>
    <template #nav-bar-content-after>
      <div class="demo-nav-tools">
        <ColorThemePicker />
        <StylePresetPicker />
      </div>
    </template>
  </LandingLayout>
</template>

<style scoped>
.demo-nav-tools {
  display: flex;
  align-items: center;
  gap: 0.125rem;
  margin-left: 0.5rem;
}
</style>
