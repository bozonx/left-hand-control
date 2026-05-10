<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'
import FieldResetButton from '~/components/shared/FieldResetButton.vue'
import { parseTextAction } from '~/types/config'
import {
    isCanonicalAction,
    isSingleKeyAction,
    normalizeActionValue,
} from '~/utils/actionSyntax'
import { validateActionValue } from '~/utils/actionValidation'

const props = withDefaults(
    defineProps<{
        allowEmpty?: boolean
        placeholder?: string
        keyOnly?: boolean
        allowMacros?: boolean
        title?: string
        clearLabel?: string
        open?: boolean
        hideTrigger?: boolean
        requireValue?: boolean
        invalid?: boolean
        excludedMacroId?: string
        excludedValues?: string[]
        excludedCategoryIds?: string[]
        ghost?: boolean
        singleKeyOnly?: boolean
    }>(),
    {
        allowMacros: true,
        placeholder: undefined,
        title: undefined,
        clearLabel: undefined,
        open: undefined,
        excludedMacroId: undefined,
        excludedValues: () => [],
        excludedCategoryIds: () => [],
        ghost: false,
    },
)

const emit = defineEmits<{
    'update:open': [value: boolean]
    apply: [value: string]
    cancel: []
    clear: []
}>()

const model = defineModel<string | null>({ default: '' })

const { getActionInfo } = useMacros()
const { config } = useConfig()

const uncontrolledOpen = ref(false)
const draft = ref('')
const originalValue = ref('')
const closeReason = ref<'apply' | 'clear' | 'cancel' | null>(null)

const actionInfo = computed(() => getActionInfo(model.value))
const isControlled = computed(() => props.open !== undefined)
const modalOpen = computed({
    get: () => (isControlled.value ? !!props.open : uncontrolledOpen.value),
    set: (value: boolean) => {
        if (isControlled.value) {
            emit('update:open', value)
            return
        }
        uncontrolledOpen.value = value
    },
})
const normalizedDraft = computed(() => {
    const raw = draft.value ?? ''
    return parseTextAction(raw) !== null ? raw : raw.trim()
})
const draftIssue = computed(() => {
    if (props.excludedValues.includes(normalizedDraft.value))
        return 'invalidSyntax'
    if (props.singleKeyOnly && !isSingleKeyAction(normalizedDraft.value))
        return 'invalidSyntax'
    if (!isCanonicalAction(normalizedDraft.value)) return 'invalidSyntax'
    return validateActionValue(normalizedDraft.value, config.value, {
        allowMacros: props.allowMacros,
        excludedMacroId: props.excludedMacroId,
    })
})
const draftInvalid = computed(() => draftIssue.value !== null)
const applyDisabled = computed(
    () => (props.requireValue && !normalizedDraft.value) || draftInvalid.value,
)
const { t } = useI18n()
const pickerTitle = computed(
    () =>
        props.title ??
        (props.keyOnly ? t('picker.titleKey') : t('picker.titleAction')),
)
const originalValueLabel = computed(() =>
    originalValue.value
        ? t('picker.currentValueWithValue', { value: originalValue.value })
        : t('picker.currentValueEmpty'),
)

watch(modalOpen, (isOpen, wasOpen) => {
    if (isOpen && !wasOpen) {
        draft.value = model.value ?? ''
        originalValue.value = model.value ?? ''
        closeReason.value = null
        return
    }

    if (!isOpen && wasOpen) {
        const reason = closeReason.value ?? 'cancel'
        if (reason === 'cancel') emit('cancel')
        if (reason === 'clear') emit('clear')
        closeReason.value = null
    }
})

function openModal() {
    modalOpen.value = true
}

function apply() {
    const next = normalizeActionValue(draft.value, props.requireValue)
    if (next === null) return
    if (props.excludedValues.includes(next)) return
    if (props.singleKeyOnly && !isSingleKeyAction(next)) return
    model.value = next
    emit('apply', next)
    closeReason.value = 'apply'
    modalOpen.value = false
}

function pickAndApply(value: string) {
    draft.value = value
    const next = normalizeActionValue(value, props.requireValue)
    if (next === null) return
    if (props.excludedValues.includes(next)) return
    if (props.singleKeyOnly && !isSingleKeyAction(next)) return
    model.value = next
    emit('apply', next)
    closeReason.value = 'apply'
    modalOpen.value = false
}

function clear() {
    draft.value = ''
    model.value = ''
    closeReason.value = 'clear'
    modalOpen.value = false
}

function cancel() {
    closeReason.value = 'cancel'
    modalOpen.value = false
}
</script>

<template>
    <div v-if="!props.hideTrigger" class="flex items-center gap-1 w-full">
        <button
            type="button"
            data-testid="action-picker-trigger"
            class="flex-1 min-w-0 h-8 px-2.5 inline-flex items-center gap-1.5 rounded-md text-sm transition-colors disabled:cursor-not-allowed disabled:opacity-75"
            :class="[
                props.ghost && !model
                    ? 'justify-start border border-dashed border-(--ui-border) text-(--ui-text-muted) hover:text-(--ui-text) hover:border-(--ui-border-accent) hover:bg-(--ui-bg-elevated)/50'
                    : 'justify-start border border-(--ui-border) bg-(--ui-bg) hover:bg-(--ui-bg-elevated)',
                props.invalid
                    ? 'border-(--ui-error) ring-1 ring-(--ui-error)'
                    : '',
            ]"
            @click="openModal"
        >
            <UIcon
                v-if="!(props.ghost && !model)"
                :name="
                    actionInfo.icon ||
                    (model ? 'i-lucide-square-mouse-pointer' : 'i-lucide-plus')
                "
                class="shrink-0 w-4 h-4 text-(--ui-text-muted)"
            />
            <AppTooltip
                v-if="actionInfo.label"
                class="truncate min-w-0"
                :text="model ?? ''"
            >
                <span>{{ actionInfo.label }}</span>
            </AppTooltip>
            <span v-else class="text-(--ui-text-muted) truncate">
                {{
                    props.ghost && !model
                        ? $t('common.notSet')
                        : (placeholder ?? $t('picker.chooseAction'))
                }}
            </span>
        </button>
        <FieldResetButton
            v-if="allowEmpty && model"
            :label="props.clearLabel ?? $t('common.clear')"
            @click="model = ''"
        />
    </div>

    <UModal
        v-model:open="modalOpen"
        fullscreen
        :ui="{ body: 'overflow-y-hidden' }"
    >
        <template #header>
            <div class="mx-auto flex w-full max-w-7xl items-center gap-3">
                <UButton
                    type="button"
                    icon="i-lucide-arrow-left"
                    color="neutral"
                    variant="ghost"
                    :aria-label="$t('common.cancel')"
                    @click="cancel"
                />
                <div class="min-w-0 flex-1">
                    <h2 class="truncate text-base font-semibold">
                        {{ pickerTitle }}
                    </h2>
                    <p class="truncate text-xs text-(--ui-text-muted)">
                        {{ originalValueLabel }}
                    </p>
                </div>
                <FieldResetButton
                    v-if="allowEmpty && (model || draft)"
                    :label="props.clearLabel ?? $t('common.clear')"
                    @click="clear"
                />
                <UButton
                    type="button"
                    color="neutral"
                    variant="ghost"
                    @click="cancel"
                >
                    {{ $t('common.cancel') }}
                </UButton>
                <UButton
                    type="button"
                    icon="i-lucide-check"
                    :disabled="applyDisabled"
                    @click="apply"
                >
                    {{ $t('common.apply') }}
                </UButton>
            </div>
        </template>

        <template #body>
            <div
                v-if="modalOpen"
                class="mx-auto flex h-full w-full max-w-7xl flex-col"
                data-testid="action-picker-view"
            >
                <ActionPickerBody
                    v-model="draft"
                    :key-only="keyOnly"
                    :single-key-only="singleKeyOnly"
                    :allow-macros="allowMacros"
                    :excluded-macro-id="excludedMacroId"
                    :excluded-values="excludedValues"
                    :excluded-category-ids="excludedCategoryIds"
                    spacious
                    @pick="pickAndApply"
                />
                <p v-if="draftInvalid" class="mt-3 text-sm text-(--ui-error)">
                    {{ $t('picker.invalidValue') }}
                </p>
            </div>
        </template>
    </UModal>
</template>
