---
layout: home
markdownStyles: false
---

<script setup lang="ts">
import { useData } from 'vitepress'

const { localeIndex } = useData()
const L = `/${localeIndex.value}`

const heroActions = [
  { text: 'Документация', link: `${L}/doc`, variant: 'brand' },
  { text: 'GitHub', link: 'https://github.com/bozonx/left-hand-control', variant: 'ghost', icon: 'fa6-brands:github' },
]

const features = [
  {
    icon: 'fa6-solid:layer-group',
    title: 'Слои и аккорды',
    text: 'Динамическое переназначение клавиш, слои по удержанию или переключению, и поддержка комплексных аккордов.',
  },
  {
    icon: 'fa6-solid:bolt',
    title: 'Макросы и действия',
    text: 'Запуск системных команд и приложений, действия рабочего стола и пошаговые макросы.',
  },
  {
    icon: 'fa6-solid:desktop',
    title: 'KDE Wayland и Windows',
    text: 'Нативная поддержка Linux KDE Plasma 6 Wayland с pop-up окнами layer-shell, а также бэкенд для Windows.',
  },
]
</script>

<LnHero
  variant="center"
  padding="lg"
  glow
  eyebrow="Клавиатурный маппер"
  title='Left Hand <span class="ln-accent">Control</span>'
  text="Перехват клавиатурного ввода, применение слоев, правил, макросов и системных команд с минимальной задержкой."
  :actions="heroActions"
/>

<LnFeatureGrid
  id="features"
  align="center"
  eyebrow="Возможности"
  title="Всё необходимое для эффективной работы с клавиатурой"
  :items="features"
/>
