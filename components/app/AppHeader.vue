<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import WindowControls from '~/components/app/WindowControls.vue'
import {
    isUserLayoutId,
    userLayoutNameFromId,
} from '~/composables/useLayoutLibrary'

const route = useRoute()
const tabsScrollRef = ref<HTMLElement | null>(null)
const contextRowRef = ref<HTMLElement | null>(null)

const { loaded, config, flush, currentLayoutId, isLayoutDirty } = useConfig()
const { saveCurrentLayout, saveBusy, openSaveAsModal } = useSettingsScreen()
const mapper = useMapper()
const { layout } = useLayout()
const gameMode = useGameMode()
const { t } = useI18n()
const { needsApproval: commandsNeedApproval } = useCommandTrust()
const { activeAutoLayoutId } = useLayoutSwitcher()

const activeLayoutId = computed<string | null | undefined>(() => {
    const settings = config.value.settings
    if (settings.layoutMode === 'auto') return activeAutoLayoutId.value
    return settings.manualActiveLayoutId ?? null
})

const activeLayoutLabel = computed<string | null>(() => {
    const id = activeLayoutId.value
    if (!id) return null
    if (isUserLayoutId(id)) return userLayoutNameFromId(id)
    return id
})

const tabItems = computed(() => [
    {
        key: 'rules',
        to: '/rules',
        label: t('tabs.rules'),
        icon: 'i-lucide-workflow',
    },
    {
        key: 'keymap',
        to: '/keymap',
        label: t('tabs.keymap'),
        icon: 'i-lucide-keyboard',
    },
    {
        key: 'macros',
        to: '/macros',
        label: t('tabs.macros'),
        icon: 'i-lucide-zap',
    },
    {
        key: 'quick-actions',
        to: '/quick-actions',
        label: t('tabs.quickActions'),
        icon: 'i-lucide-zap',
    },
    {
        key: 'emoji',
        to: '/emoji',
        label: t('tabs.emoji'),
        icon: 'i-lucide-smile',
    },
    {
        key: 'commands',
        to: '/commands',
        label: t('tabs.commands'),
        icon: 'i-lucide-terminal',
        attention: commandsNeedApproval.value,
    },
])

const currentLayoutLabel = computed<string>(() => {
    const id = currentLayoutId.value
    if (!id) {
        // In auto mode, an empty id means "no layout matched" rather than a
        // custom unsaved layout.
        return config.value.settings.layoutMode === 'auto'
            ? t('app.noLayout')
            : t('app.customLayout')
    }
    if (isUserLayoutId(id)) return userLayoutNameFromId(id)
    return id
})

const selectedDevice = computed(
    () => config.value.settings.inputDevicePath ?? '',
)
const selectedMouse = computed(
    () => config.value.settings.inputMouseDevicePath ?? '',
)
const saveTooltip = computed(() =>
    isLayoutDirty.value
        ? t('app.saveLayoutTooltip', { name: currentLayoutLabel.value })
        : t('app.saved'),
)
const gameModeTooltip = computed(() => {
    if (!gameMode.status.value.active) return t('settings.gameModeInactive')
    return gameMode.status.value.method
        ? `${t('settings.gameModeActive')}: ${gameMode.status.value.method}`
        : t('settings.gameModeActive')
})

function isActive(path: string) {
    return route.path === path
}

function onTabWheel(e: WheelEvent) {
    if (!tabsScrollRef.value) return
    const el = tabsScrollRef.value
    if (e.deltaY !== 0 && el.scrollWidth > el.clientWidth) {
        e.preventDefault()
        el.scrollLeft += e.deltaY
    }
}

async function openTab(path: string) {
    if (isActive(path)) return
    await navigateTo(path)
}

async function toggleMapper() {
    try {
        await flush()
        if (mapper.status.value.running) {
            await mapper.stop()
            return
        }
        if (!selectedDevice.value) return
        await mapper.start(
            selectedDevice.value,
            selectedMouse.value || undefined,
        )
    } catch (error) {
        mapper.error.value =
            error instanceof Error ? error.message : String(error)
    }
}

watch(currentLayoutId, () => {
    const el = contextRowRef.value
    if (!el) return
    el.classList.remove('layout-context-flash')
    // Force reflow so the animation restarts when triggered repeatedly.
    void el.offsetWidth
    el.classList.add('layout-context-flash')
})

onMounted(() => {
    void mapper.refreshStatus()
})
</script>

<template>
    <header
        class="flex flex-col shrink-0 app-chrome border-b border-(--ui-border) bg-(--ui-bg-elevated)"
    >
        <!-- Row 1: app chrome — drag region, global controls and status.
             `data-tauri-drag-region` only drags when the click lands on the
             element that carries it (Tauri matches the exact target), so it is
             repeated on the row's filler areas; interactive children stay
             clickable because they are not the drag-region element. -->
        <div
            class="flex items-center justify-between px-4 gap-3 h-[var(--app-chrome-height)]"
            data-tauri-drag-region
        >
            <div class="flex items-center gap-2 min-w-0" data-tauri-drag-region>
                <WindowControls side="left" />
                <UButton
                    color="neutral"
                    variant="ghost"
                    :square="true"
                    size="sm"
                    icon="i-lucide-keyboard"
                    :aria-label="$t('app.title')"
                    class="shrink-0 -ml-2 transition-all duration-200"
                    :class="
                        isActive('/')
                            ? 'text-primary bg-(--ui-primary)/8'
                            : 'text-(--ui-text-muted) hover:text-primary'
                    "
                    to="/"
                />
                <span
                    class="text-sm font-semibold text-(--ui-text-highlighted) truncate"
                    data-tauri-drag-region
                >
                    {{ $t('app.title') }}
                </span>

                <span
                    v-if="loaded"
                    class="shrink-0 mx-1 h-5 w-px bg-(--ui-border)"
                    aria-hidden="true"
                />

                <AppTooltip v-if="loaded" :text="saveTooltip">
                    <UButton
                        icon="i-lucide-save"
                        size="sm"
                        :color="isLayoutDirty ? 'primary' : 'neutral'"
                        :variant="isLayoutDirty ? 'solid' : 'outline'"
                        :loading="saveBusy"
                        :disabled="!isLayoutDirty"
                        @click="saveCurrentLayout"
                    >
                        {{ $t('common.save') }}
                    </UButton>
                </AppTooltip>

                <AppTooltip v-if="loaded" :text="$t('settings.saveAs')">
                    <UButton
                        icon="i-lucide-copy"
                        size="sm"
                        color="neutral"
                        variant="outline"
                        :disabled="!currentLayoutId"
                        @click="openSaveAsModal"
                    >
                        {{ $t('settings.saveAs') }}
                    </UButton>
                </AppTooltip>
            </div>

            <div
                class="flex items-center gap-3 shrink-0"
                data-tauri-drag-region
            >
                <template v-if="loaded">
                    <AppTooltip
                        :disabled="
                            mapper.status.value.running || !!selectedDevice
                        "
                        :text="$t('settings.startDisabledTooltip')"
                    >
                        <UButton
                            :color="
                                mapper.status.value.running
                                    ? 'error'
                                    : 'primary'
                            "
                            :variant="
                                mapper.status.value.running ? 'soft' : 'solid'
                            "
                            :icon="
                                mapper.status.value.running
                                    ? 'i-lucide-square'
                                    : 'i-lucide-play'
                            "
                            size="sm"
                            class="whitespace-nowrap"
                            :loading="mapper.busy.value"
                            :disabled="
                                !mapper.status.value.running && !selectedDevice
                            "
                            @click="toggleMapper"
                        >
                            {{
                                mapper.status.value.running
                                    ? $t('settings.stop')
                                    : $t('settings.start')
                            }}
                        </UButton>
                    </AppTooltip>

                    <AppTooltip
                        v-if="activeLayoutLabel"
                        :text="$t('app.activeLayoutTooltip')"
                    >
                        <UBadge
                            :color="
                                mapper.status.value.running
                                    ? 'primary'
                                    : 'neutral'
                            "
                            :variant="
                                mapper.status.value.running ? 'soft' : 'outline'
                            "
                            size="sm"
                            :class="
                                !mapper.status.value.running ? 'opacity-50' : ''
                            "
                        >
                            <UIcon
                                name="i-lucide-radio"
                                class="mr-1 shrink-0"
                            />
                            {{ activeLayoutLabel }}
                        </UBadge>
                    </AppTooltip>

                    <AppTooltip v-if="layout" :text="layout.long">
                        <UBadge
                            color="neutral"
                            variant="outline"
                            size="sm"
                            class="font-mono uppercase"
                        >
                            <UIcon
                                name="i-lucide-languages"
                                class="mr-1 shrink-0"
                            />
                            <span class="text-xs opacity-60 mr-0.5">{{
                                $t('app.layoutLanguageLabel')
                            }}</span>
                            {{ layout.short
                            }}{{ layout.display ? ` (${layout.display})` : '' }}
                        </UBadge>
                    </AppTooltip>

                    <AppTooltip :text="gameModeTooltip">
                        <UBadge
                            :color="
                                gameMode.status.value.active
                                    ? 'error'
                                    : 'neutral'
                            "
                            :variant="
                                gameMode.status.value.active
                                    ? 'solid'
                                    : 'outline'
                            "
                            size="sm"
                            :class="
                                !gameMode.status.value.active
                                    ? 'opacity-50'
                                    : ''
                            "
                        >
                            <UIcon
                                name="i-lucide-gamepad-2"
                                class="mr-1 h-3.5 w-3.5"
                            />
                            {{ $t('app.gameModeLabel') }}
                        </UBadge>
                    </AppTooltip>

                    <AppTooltip :text="$t('app.settingsTooltip')">
                        <UButton
                            :color="
                                isActive('/settings') ? 'primary' : 'neutral'
                            "
                            :variant="isActive('/settings') ? 'soft' : 'ghost'"
                            icon="i-lucide-settings"
                            size="sm"
                            :aria-label="$t('tabs.settings')"
                            @click="openTab('/settings')"
                        />
                    </AppTooltip>
                </template>
                <div
                    v-else
                    class="text-xs text-(--ui-text-muted)"
                    data-tauri-drag-region
                >
                    {{ $t('app.loading') }}
                </div>
                <WindowControls side="right" />
            </div>
        </div>

        <!-- Row 2: layout context — breadcrumb to layouts + current name + edit tabs -->
        <div
            ref="contextRowRef"
            class="flex items-center px-4 gap-2 h-[var(--app-context-height)] border-t border-(--ui-border)/60 bg-(--ui-bg)"
        >
            <UButton
                color="neutral"
                variant="ghost"
                size="sm"
                :icon="isActive('/') ? 'i-lucide-home' : 'i-lucide-arrow-left'"
                class="shrink-0 -ml-2 transition-all duration-200"
                :class="
                    isActive('/')
                        ? 'text-primary bg-(--ui-primary)/8'
                        : 'text-(--ui-text-muted) hover:text-primary'
                "
                to="/"
            >
                {{ $t('app.layoutsBack') }}
            </UButton>

            <span
                class="shrink-0 text-(--ui-text-muted) opacity-50 select-none"
                aria-hidden="true"
                >/</span
            >

            <AppTooltip :text="currentLayoutLabel">
                <span
                    class="block shrink min-w-0 max-w-[14rem] truncate px-1 text-sm font-semibold text-(--ui-text-highlighted)"
                >
                    {{ currentLayoutLabel }}
                </span>
            </AppTooltip>

            <AppTooltip v-if="isLayoutDirty" :text="$t('app.dirtyTooltip')">
                <span
                    class="shrink-0 inline-block size-2 rounded-full bg-(--ui-primary)"
                    aria-hidden="true"
                />
            </AppTooltip>

            <span
                class="shrink-0 mx-1 h-5 w-px bg-(--ui-border)"
                aria-hidden="true"
            />

            <div
                ref="tabsScrollRef"
                class="min-w-0 flex-1 overflow-x-auto overflow-y-hidden hide-scrollbar"
                @wheel="onTabWheel"
            >
                <div class="inline-flex min-w-max items-center gap-1.5">
                    <UButton
                        v-for="item in tabItems"
                        :key="item.key"
                        :color="isActive(item.to) ? 'primary' : 'neutral'"
                        variant="ghost"
                        :icon="item.icon"
                        :aria-label="item.label"
                        size="sm"
                        class="relative px-3 text-(--ui-text-muted) hover:text-primary"
                        :class="
                            isActive(item.to)
                                ? 'text-primary shadow-none ring-1 ring-inset ring-(--ui-primary)/25 bg-(--ui-primary)/8'
                                : 'bg-transparent shadow-none'
                        "
                        :to="item.to"
                    >
                        <span>{{ item.label }}</span>
                        <span
                            v-if="item.attention"
                            class="absolute right-1.5 top-1.5 size-1.5 rounded-full bg-(--ui-warning)"
                            aria-hidden="true"
                        />
                    </UButton>
                </div>
            </div>
        </div>
    </header>
</template>
