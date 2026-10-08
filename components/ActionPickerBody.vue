<script setup lang="ts">
import ActionPickerCategoryPanel from '~/components/features/action-picker/ActionPickerCategoryPanel.vue'
import ActionPickerCategoryTabs from '~/components/features/action-picker/ActionPickerCategoryTabs.vue'
import ActionPickerValueField from '~/components/features/action-picker/ActionPickerValueField.vue'
import {
    appActionRef,
    commandActionRef,
    type Command,
    macroActionRef,
    parseAppRef,
    parseCommandRef,
    parseMacroRef,
    parseSystemRef,
    parseTextAction,
    systemActionRef,
} from '~/types/config'
import {
    STATIC_CATEGORIES,
    type ActionItem,
    type StaticCategory,
} from '~/utils/actionCategories'
import { APP_ACTIONS } from '~/utils/appActions'
import { SYSTEM_ACTIONS } from '~/utils/systemActions'
import { SYSTEM_MACROS } from '~/utils/systemMacros'

const props = withDefaults(
    defineProps<{
        keyOnly?: boolean
        commandOnly?: boolean
        spacious?: boolean
        excludedMacroId?: string
        excludedValues?: string[]
        excludedCategoryIds?: string[]
        allowMacros?: boolean
        singleKeyOnly?: boolean
    }>(),
    {
        allowMacros: true,
        excludedMacroId: undefined,
        excludedValues: () => [],
        excludedCategoryIds: () => [],
    },
)

const emit = defineEmits<{
    pick: [value: string]
}>()

const draft = defineModel<string>({ default: '' })
const commandDraft = defineModel<Command | null>('command', { default: null })
const { config } = useConfig()
const selectedCommand = computed(() => commands.value.find((command) => command.id === parseCommandRef(draft.value)))
function editCommand(field: 'name' | 'linux' | 'workingDirectory', value: string) {
    const command = commandDraft.value?.id === parseCommandRef(draft.value) ? commandDraft.value : selectedCommand.value
    commandDraft.value = { ...(command ?? { id: crypto.randomUUID(), name: '', linux: '' }), [field]: value }
    draft.value = commandActionRef(commandDraft.value.id)
}
function newCommand() {
    commandDraft.value = { id: crypto.randomUUID(), name: '', linux: '' }
    draft.value = commandActionRef(commandDraft.value.id)
}
const editingCommand = computed(() => commandDraft.value?.id === parseCommandRef(draft.value) ? commandDraft.value : selectedCommand.value)
const selectionVersion = ref(0)

const { macros } = useMacros()
const { commands } = useCommands()
const { t } = useI18n()

const dynamicCategories = computed<StaticCategory[]>(() => {
    const userMacros = macros.value.filter(
        (m) => m.id !== props.excludedMacroId,
    )
    const userIds = new Set(userMacros.map((m) => m.id))
    return [
        ...(!config.value.settings.commandsEnabled
            ? []
            : [
                  {
                      id: 'commands',
                      labelKey: 'categories.commands',
                      icon: 'i-lucide-terminal',
                      items: commands.value.map((command) => ({
                          label: command.name || command.id,
                          value: commandActionRef(command.id),
                          hint: command.linux,
                      })),
                  },
              ]),
        ...(props.allowMacros === false
            ? []
            : [
                  {
                      id: 'macros',
                      labelKey: 'categories.macros',
                      icon: 'i-lucide-zap',
                      items: userMacros.map((m) => ({
                          label: m.name || m.id,
                          value: macroActionRef(m.id),
                          hint: m.id,
                      })),
                  },
                  {
                      id: 'system-macros',
                      labelKey: 'categories.systemMacros',
                      icon: 'i-lucide-cpu',
                      items: SYSTEM_MACROS.filter(
                          (m) =>
                              !userIds.has(m.id) &&
                              m.id !== props.excludedMacroId,
                      ).map((m) => ({
                          label: m.name,
                          value: macroActionRef(m.id),
                          hint: m.id,
                      })),
                  },
              ]),
        {
            id: 'app',
            labelKey: 'categories.app',
            icon: 'i-lucide-app-window',
            items: APP_ACTIONS.map((action) => ({
                label: t(action.nameKey),
                value: appActionRef(action.id),
                hint: action.id,
            })),
        },
        {
            id: 'system',
            labelKey: 'categories.system',
            icon: 'i-lucide-settings-2',
            items: SYSTEM_ACTIONS.map((action) => ({
                label: t(action.nameKey, action.nameParams ?? {}),
                value: systemActionRef(action.id),
                hint: action.id,
            })),
        },
    ]
})

const allCategories = computed<StaticCategory[]>(() =>
    (props.keyOnly
        ? STATIC_CATEGORIES
        : [...dynamicCategories.value, ...STATIC_CATEGORIES])
        .filter((category) => (!props.commandOnly || category.id === 'commands') && !props.excludedCategoryIds.includes(category.id))
        .map((category) => ({
            ...category,
            items: category.items.filter(
                (item) => !props.excludedValues.includes(item.value),
            ),
        })),
)

const activeCategory = ref<string>(allCategories.value[0]?.id ?? 'special')
const textCategoryAvailable = computed(
    () => !props.commandOnly && !props.keyOnly && !props.excludedCategoryIds.includes('text'),
)

function categoryAvailable(id: string, cats = allCategories.value) {
    if (id === 'text') return textCategoryAvailable.value
    return cats.some((category) => category.id === id)
}

function detectCategory(value: string): string | null {
    if (!value) return null

    if (parseTextAction(value) !== null) {
        return textCategoryAvailable.value ? 'text' : null
    }

    const commandId = parseCommandRef(value)
    if (commandId !== null) {
        return config.value.settings.commandsEnabled ? 'commands' : null
    }

    const macroId = parseMacroRef(value)
    if (macroId !== null) {
        if (props.allowMacros === false) return null
        const userMacro = macros.value.find(
            (m) => m.id === macroId && m.id !== props.excludedMacroId,
        )
        if (userMacro) return 'macros'
        const sysMacro = SYSTEM_MACROS.find(
            (m) => m.id === macroId && m.id !== props.excludedMacroId,
        )
        if (sysMacro) return 'system-macros'
        return 'macros'
    }

    if (parseSystemRef(value) !== null) return 'system'

    if (parseAppRef(value) !== null) return 'app'

    for (const cat of STATIC_CATEGORIES) {
        if (cat.items.some((item) => item.value === value)) return cat.id
    }

    return 'special'
}

watchEffect(() => {
    const val = draft.value
    const cats = allCategories.value

    if (!val) {
        if (!categoryAvailable(activeCategory.value, cats)) {
            activeCategory.value = cats[0]?.id ?? 'special'
        }
        return
    }

    const detected = detectCategory(val)
    if (detected && categoryAvailable(detected, cats)) {
        activeCategory.value = detected
    } else if (!categoryAvailable(activeCategory.value, cats)) {
        activeCategory.value = cats[0]?.id ?? 'special'
    }
})

const categoryItems = computed(() => {
    const category = allCategories.value.find(
        (item) => item.id === activeCategory.value,
    )
    return category?.items ?? []
})

const filteredItems = computed(() => {
    const query = draft.value.trim().toLowerCase()
    if (!query) return []
    const allItems: ActionItem[] = []
    for (const category of allCategories.value) {
        for (const item of category.items) {
            const searchable = (item.hint || item.value).toLowerCase()
            if (searchable.includes(query)) {
                allItems.push(item)
            }
        }
    }
    return allItems
})

function pickValue(value: string) {
    draft.value = value
    selectionVersion.value += 1
    emit('pick', value)
}

function pickItem(item: ActionItem) {
    pickValue(item.value)
}
</script>

<template>
    <div
        :class="
            props.spacious ? 'min-h-0 flex flex-1 flex-col gap-4' : 'space-y-4'
        "
    >
        <ActionPickerValueField
            v-if="activeCategory !== 'commands' && !(parseCommandRef(draft) && !config.settings.commandsEnabled)"
            v-model="draft"
            :active-category="activeCategory"
            :filtered-items="filteredItems"
            :key-only="props.keyOnly"
            :selection-version="selectionVersion"
            :single-key-only="props.singleKeyOnly"
            @pick="pickValue"
        />

        <ActionPickerCategoryTabs
            v-model:active-category="activeCategory"
            :categories="allCategories"
            :key-only="props.keyOnly"
            :show-text-category="textCategoryAvailable"
        />

        <p v-if="parseCommandRef(draft) && !config.settings.commandsEnabled" class="text-sm text-(--ui-text-muted)">
            {{ selectedCommand?.name || selectedCommand?.linux || draft }} — {{ $t('commands.disabled') }}
        </p>
        <div v-if="activeCategory === 'commands' && config.settings.commandsEnabled" class="space-y-3">
            <UInput :model-value="editingCommand?.name ?? ''" :placeholder="$t('commands.namePh')" class="w-full" @update:model-value="editCommand('name', $event)" />
            <UTextarea :model-value="editingCommand?.linux ?? ''" :placeholder="$t('commands.script')" class="w-full" :rows="4" @update:model-value="editCommand('linux', $event)" />
            <UInput :model-value="editingCommand?.workingDirectory ?? ''" :placeholder="$t('commands.workingDirectory')" class="w-full" @update:model-value="editCommand('workingDirectory', $event)" />
            <UButton v-if="!props.commandOnly" color="neutral" variant="outline" @click="newCommand">{{ $t('commands.addBtn') }}</UButton>
        </div>

        <ActionPickerCategoryPanel
            v-if="!props.commandOnly"
            :active-category="activeCategory"
            :draft="draft"
            :items="categoryItems"
            :spacious="props.spacious"
            @pick="pickItem"
        />
    </div>
</template>
