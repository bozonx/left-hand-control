<script setup lang="ts">
import ActionPickerCategoryPanel from '~/components/features/action-picker/ActionPickerCategoryPanel.vue'
import ActionPickerCategoryTabs from '~/components/features/action-picker/ActionPickerCategoryTabs.vue'
import ActionPickerValueField from '~/components/features/action-picker/ActionPickerValueField.vue'
import {
    commandActionRef,
    macroActionRef,
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
import { SYSTEM_ACTIONS } from '~/utils/systemActions'
import { SYSTEM_MACROS } from '~/utils/systemMacros'

const props = withDefaults(
    defineProps<{
        keyOnly?: boolean
        spacious?: boolean
        excludedMacroId?: string
        allowMacros?: boolean
        singleKeyOnly?: boolean
    }>(),
    {
        allowMacros: true,
        excludedMacroId: undefined,
    },
)

const emit = defineEmits<{
    pick: [value: string]
}>()

const draft = defineModel<string>({ default: '' })

const { macros } = useMacros()
const { commands } = useCommands()
const { t } = useI18n()

const dynamicCategories = computed<StaticCategory[]>(() => {
    const userMacros = macros.value.filter(
        (m) => m.id !== props.excludedMacroId,
    )
    const userIds = new Set(userMacros.map((m) => m.id))
    return [
        ...(commands.value.length === 0
            ? []
            : [
                  {
                      id: 'commands',
                      labelKey: 'categories.commands',
                      icon: 'i-lucide-terminal',
                      items: commands.value.map((command) => ({
                          label: command.name || command.id,
                          value: commandActionRef(command.id),
                          hint: command.id,
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
    props.keyOnly
        ? STATIC_CATEGORIES
        : [...dynamicCategories.value, ...STATIC_CATEGORIES],
)

const activeCategory = ref<string>(allCategories.value[0]?.id ?? 'special')

function detectCategory(value: string): string | null {
    if (!value) return null

    if (parseTextAction(value) !== null) return 'text'

    const commandId = parseCommandRef(value)
    if (commandId !== null) {
        return commands.value.length > 0 ? 'commands' : null
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

    for (const cat of STATIC_CATEGORIES) {
        if (cat.items.some((item) => item.value === value)) return cat.id
    }

    return 'special'
}

watchEffect(() => {
    const val = draft.value
    const cats = allCategories.value

    if (!val) {
        if (!cats.some((c) => c.id === activeCategory.value)) {
            activeCategory.value = cats[0]?.id ?? 'special'
        }
        return
    }

    const detected = detectCategory(val)
    if (detected && cats.some((c) => c.id === detected)) {
        activeCategory.value = detected
    } else if (!cats.some((c) => c.id === activeCategory.value)) {
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
            v-model="draft"
            :active-category="activeCategory"
            :filtered-items="filteredItems"
            :key-only="props.keyOnly"
            :single-key-only="props.singleKeyOnly"
            @pick="pickValue"
        />

        <ActionPickerCategoryTabs
            v-model:active-category="activeCategory"
            :categories="allCategories"
            :key-only="props.keyOnly"
        />

        <ActionPickerCategoryPanel
            :active-category="activeCategory"
            :draft="draft"
            :items="categoryItems"
            :spacious="props.spacious"
            @pick="pickItem"
        />
    </div>
</template>
