<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import FieldLabel from '~/components/FieldLabel.vue'
import type { GameModeProcessMatcher } from '~/types/config'

const props = defineProps<{
    useGamemoded: boolean
    useFullscreen: boolean
    processMatchers: GameModeProcessMatcher[]
}>()

const emit = defineEmits<{
    'update:useGamemoded': [value: boolean]
    'update:useFullscreen': [value: boolean]
    'update:processMatchers': [value: GameModeProcessMatcher[]]
}>()

const advancedOpen = ref(false)

const whitelistMatchers = computed(() =>
    props.processMatchers.filter((item) => !item.isBlacklist),
)
const blacklistMatchers = computed(() =>
    props.processMatchers.filter((item) => item.isBlacklist),
)

function updateMatcher(id: string, patch: Partial<GameModeProcessMatcher>) {
    emit(
        'update:processMatchers',
        props.processMatchers.map((item) =>
            item.id === id ? { ...item, ...patch } : item,
        ),
    )
}

function removeMatcher(id: string) {
    emit(
        'update:processMatchers',
        props.processMatchers.filter((item) => item.id !== id),
    )
}

function addMatcher(isBlacklist: boolean) {
    emit('update:processMatchers', [
        ...props.processMatchers,
        {
            id: `process-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
            name: '',
            onlyActiveWindow: false,
            isBlacklist,
        },
    ])
}

function moveMatcher(id: string, isBlacklist: boolean, direction: -1 | 1) {
    const group = props.processMatchers.filter(
        (item) => !!item.isBlacklist === isBlacklist,
    )
    const index = group.findIndex((item) => item.id === id)
    const nextIndex = index + direction
    if (index < 0 || nextIndex < 0 || nextIndex >= group.length) return

    const reordered = [...group]
    const [item] = reordered.splice(index, 1)
    reordered.splice(nextIndex, 0, item)

    let groupIndex = 0
    emit(
        'update:processMatchers',
        props.processMatchers.map((item) =>
            !!item.isBlacklist === isBlacklist
                ? reordered[groupIndex++]
                : item,
        ),
    )
}
</script>

<template>
    <UCard variant="outline">
        <template #header>
            <div>
                <div class="flex items-center gap-2">
                    <UIcon
                        name="i-lucide-gamepad-2"
                        class="h-5 w-5 text-(--ui-primary)"
                    />
                    <h2 class="text-sm font-semibold">
                        {{ $t('settings.gameModeTitle') }}
                    </h2>
                    <AppTooltip
                        :text="$t('settings.gameModeTitleHint')"
                        align="start"
                        toggle-on-click
                    >
                        <UIcon
                            name="i-lucide-info"
                            class="h-4 w-4 cursor-help text-(--ui-text-muted)"
                        />
                    </AppTooltip>
                </div>
                <p class="mt-1 text-sm text-(--ui-text-muted)">
                    {{ $t('settings.gameModeSubtitle') }}
                </p>
            </div>
        </template>

        <div class="space-y-4">
            <div class="flex flex-wrap items-center gap-4">
                <div class="flex items-center gap-2">
                    <USwitch
                        :model-value="useGamemoded"
                        @update:model-value="
                            $emit('update:useGamemoded', $event as boolean)
                        "
                    />
                    <div>
                        <FieldLabel
                            :label="$t('settings.gameModeUseGamemoded')"
                            :hint="$t('settings.gameModeUseGamemodedHint')"
                        />
                    </div>
                </div>

                <div class="flex items-center gap-2">
                    <USwitch
                        :model-value="useFullscreen"
                        @update:model-value="
                            $emit('update:useFullscreen', $event as boolean)
                        "
                    />
                    <div>
                        <FieldLabel
                            :label="$t('settings.gameModeUseFullscreen')"
                            :hint="$t('settings.gameModeUseFullscreenHint')"
                        />
                    </div>
                </div>
            </div>

            <UButton
                type="button"
                variant="link"
                color="primary"
                :icon="
                    advancedOpen
                        ? 'i-lucide-chevron-down'
                        : 'i-lucide-chevron-right'
                "
                class="px-0"
                @click="advancedOpen = !advancedOpen"
            >
                {{ $t('settings.gameModeAdvanced') }}
            </UButton>

            <div
                v-if="advancedOpen"
                class="space-y-6 border-t border-(--ui-border) pt-4"
            >
                <div class="space-y-4 rounded-lg border border-(--ui-border) p-4">
                    <div class="flex items-center gap-2">
                        <h3 class="text-sm font-medium">
                            {{ $t('settings.gameModeListsTitle') }}
                        </h3>
                        <AppTooltip
                            :text="$t('settings.gameModeListsHint')"
                            align="start"
                            toggle-on-click
                        >
                            <UIcon
                                name="i-lucide-info"
                                class="h-4 w-4 cursor-help text-(--ui-text-muted)"
                            />
                        </AppTooltip>
                    </div>

                    <div class="space-y-3">
                        <div class="flex items-center justify-between gap-3">
                            <h4 class="text-sm font-medium">
                                {{ $t('settings.gameModeWhitelistTitle') }}
                            </h4>
                            <UButton
                                type="button"
                                icon="i-lucide-plus"
                                size="sm"
                                @click="addMatcher(false)"
                            >
                                {{ $t('settings.gameModeAddProcess') }}
                            </UButton>
                        </div>

                        <div v-if="whitelistMatchers.length > 0" class="space-y-2">
                            <div
                                v-for="(item, index) in whitelistMatchers"
                                :key="item.id"
                                class="grid gap-2 rounded-lg border border-(--ui-border) p-3 lg:grid-cols-[minmax(0,1fr)_13rem_auto] lg:items-center"
                            >
                                <div class="flex min-w-0 items-center gap-2">
                                    <UInput
                                        :model-value="item.name"
                                        class="min-w-0 flex-1"
                                        :color="
                                            item.name.trim()
                                                ? 'neutral'
                                                : 'error'
                                        "
                                        :placeholder="
                                            $t(
                                                'settings.gameModeProcessPlaceholder',
                                            )
                                        "
                                        :aria-invalid="!item.name.trim()"
                                        @update:model-value="
                                            updateMatcher(item.id, {
                                                name: String($event),
                                            })
                                        "
                                    />
                                    <AppTooltip
                                        :text="$t('settings.gameModeProcessNameHint')"
                                        toggle-on-click
                                    >
                                        <UIcon
                                            name="i-lucide-info"
                                            class="h-4 w-4 shrink-0 cursor-help text-(--ui-text-muted)"
                                        />
                                    </AppTooltip>
                                </div>
                                <div class="flex items-center gap-2">
                                    <UCheckbox
                                        :model-value="item.onlyActiveWindow"
                                        :label="
                                            $t(
                                                'settings.gameModeOnlyActiveWindow',
                                            )
                                        "
                                        @update:model-value="
                                            updateMatcher(item.id, {
                                                onlyActiveWindow:
                                                    $event as boolean,
                                            })
                                        "
                                    />
                                    <AppTooltip
                                        :text="
                                            $t(
                                                'settings.gameModeOnlyActiveWindowHint',
                                            )
                                        "
                                        toggle-on-click
                                    >
                                        <UIcon
                                            name="i-lucide-info"
                                            class="h-4 w-4 shrink-0 cursor-help text-(--ui-text-muted)"
                                        />
                                    </AppTooltip>
                                </div>
                                <div class="flex items-center justify-end gap-1">
                                    <AppTooltip :text="$t('common.moveUp')">
                                        <UButton
                                            type="button"
                                            color="neutral"
                                            variant="ghost"
                                            icon="i-lucide-arrow-up"
                                            :disabled="index === 0"
                                            :aria-label="$t('common.moveUp')"
                                            @click="moveMatcher(item.id, false, -1)"
                                        />
                                    </AppTooltip>
                                    <AppTooltip :text="$t('common.moveDown')">
                                        <UButton
                                            type="button"
                                            color="neutral"
                                            variant="ghost"
                                            icon="i-lucide-arrow-down"
                                            :disabled="
                                                index ===
                                                whitelistMatchers.length - 1
                                            "
                                            :aria-label="$t('common.moveDown')"
                                            @click="moveMatcher(item.id, false, 1)"
                                        />
                                    </AppTooltip>
                                    <AppTooltip
                                        :text="
                                            $t(
                                                'settings.gameModeRemoveProcess',
                                            )
                                        "
                                    >
                                        <UButton
                                            type="button"
                                            color="neutral"
                                            variant="ghost"
                                            icon="i-lucide-trash-2"
                                            :aria-label="
                                                $t(
                                                    'settings.gameModeRemoveProcess',
                                                )
                                            "
                                            @click="removeMatcher(item.id)"
                                        />
                                    </AppTooltip>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="space-y-3">
                        <div class="flex items-center justify-between gap-3">
                            <h4 class="text-sm font-medium">
                                {{ $t('settings.gameModeBlacklistTitle') }}
                            </h4>
                            <UButton
                                type="button"
                                icon="i-lucide-plus"
                                size="sm"
                                @click="addMatcher(true)"
                            >
                                {{ $t('settings.gameModeAddProcess') }}
                            </UButton>
                        </div>

                        <div v-if="blacklistMatchers.length > 0" class="space-y-2">
                            <div
                                v-for="(item, index) in blacklistMatchers"
                                :key="item.id"
                                class="grid gap-2 rounded-lg border border-(--ui-border) bg-(--ui-bg-muted)/50 p-3 lg:grid-cols-[minmax(0,1fr)_13rem_auto] lg:items-center"
                            >
                                <div class="flex min-w-0 items-center gap-2">
                                    <UInput
                                        :model-value="item.name"
                                        class="min-w-0 flex-1"
                                        :color="
                                            item.name.trim()
                                                ? 'neutral'
                                                : 'error'
                                        "
                                        :placeholder="
                                            $t(
                                                'settings.gameModeProcessPlaceholder',
                                            )
                                        "
                                        :aria-invalid="!item.name.trim()"
                                        @update:model-value="
                                            updateMatcher(item.id, {
                                                name: String($event),
                                            })
                                        "
                                    />
                                    <AppTooltip
                                        :text="$t('settings.gameModeProcessNameHint')"
                                        toggle-on-click
                                    >
                                        <UIcon
                                            name="i-lucide-info"
                                            class="h-4 w-4 shrink-0 cursor-help text-(--ui-text-muted)"
                                        />
                                    </AppTooltip>
                                </div>
                                <div class="flex items-center gap-2">
                                    <UCheckbox
                                        :model-value="item.onlyActiveWindow"
                                        :label="
                                            $t(
                                                'settings.gameModeOnlyActiveWindow',
                                            )
                                        "
                                        @update:model-value="
                                            updateMatcher(item.id, {
                                                onlyActiveWindow:
                                                    $event as boolean,
                                            })
                                        "
                                    />
                                    <AppTooltip
                                        :text="
                                            $t(
                                                'settings.gameModeOnlyActiveWindowHint',
                                            )
                                        "
                                        toggle-on-click
                                    >
                                        <UIcon
                                            name="i-lucide-info"
                                            class="h-4 w-4 shrink-0 cursor-help text-(--ui-text-muted)"
                                        />
                                    </AppTooltip>
                                </div>
                                <div class="flex items-center justify-end gap-1">
                                    <AppTooltip :text="$t('common.moveUp')">
                                        <UButton
                                            type="button"
                                            color="neutral"
                                            variant="ghost"
                                            icon="i-lucide-arrow-up"
                                            :disabled="index === 0"
                                            :aria-label="$t('common.moveUp')"
                                            @click="moveMatcher(item.id, true, -1)"
                                        />
                                    </AppTooltip>
                                    <AppTooltip :text="$t('common.moveDown')">
                                        <UButton
                                            type="button"
                                            color="neutral"
                                            variant="ghost"
                                            icon="i-lucide-arrow-down"
                                            :disabled="
                                                index ===
                                                blacklistMatchers.length - 1
                                            "
                                            :aria-label="$t('common.moveDown')"
                                            @click="moveMatcher(item.id, true, 1)"
                                        />
                                    </AppTooltip>
                                    <AppTooltip
                                        :text="
                                            $t(
                                                'settings.gameModeRemoveProcess',
                                            )
                                        "
                                    >
                                        <UButton
                                            type="button"
                                            color="neutral"
                                            variant="ghost"
                                            icon="i-lucide-trash-2"
                                            :aria-label="
                                                $t(
                                                    'settings.gameModeRemoveProcess',
                                                )
                                            "
                                            @click="removeMatcher(item.id)"
                                        />
                                    </AppTooltip>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </UCard>
</template>
