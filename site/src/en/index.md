---
layout: home
markdownStyles: false
---

<script setup lang="ts">
import { useData } from 'vitepress'

const { localeIndex } = useData()
const L = `/${localeIndex.value}`

const heroActions = [
  { text: 'Documentation', link: `${L}/doc`, variant: 'brand' },
  { text: 'GitHub', link: 'https://github.com/bozonx/left-hand-control', variant: 'ghost', icon: 'fa6-brands:github' },
]

const features = [
  {
    icon: 'fa6-solid:layer-group',
    title: 'Layers & Chords',
    text: 'Remap keys dynamically, define layers activated by holding or tapping keys, and create complex chords.',
  },
  {
    icon: 'fa6-solid:bolt',
    title: 'Macros & Actions',
    text: 'Execute shell commands, launch applications, trigger desktop system actions, and run sequenced macros.',
  },
  {
    icon: 'fa6-solid:desktop',
    title: 'KDE Wayland & Windows',
    text: 'First-class support for Linux KDE Plasma 6 Wayland with layer-shell popups, plus Windows mapper backend.',
  },
]
</script>

<LnHero
  variant="center"
  padding="lg"
  glow
  eyebrow="Desktop Keyboard Mapper"
  title='Left Hand <span class="ln-accent">Control</span>'
  text="Intercept physical keyboard input, apply layers, rules, macros and commands with sub-millisecond latency."
  :actions="heroActions"
/>

<LnFeatureGrid
  id="features"
  align="center"
  eyebrow="Features"
  title="Everything you need for keyboard mastery"
  :items="features"
/>
