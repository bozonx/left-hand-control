<script setup lang="ts">
import AppTooltip from '~/components/shared/AppTooltip.vue'

const props = defineProps<{
    selectedLayerId: string
    layerItems: Array<{ label: string; value: string }>
    currentLayerName?: string
    currentLayerDescription?: string
}>()

const emit = defineEmits<{
    'update:selectedLayerId': [value: string]
    create: []
    clone: []
    rename: []
    'update-description': [value: string]
    delete: []
}>()

const isDescriptionEditing = ref(false)
const descriptionDraft = ref('')
const descriptionInput = useTemplateRef('descriptionInput')
const testInput = ref('')

watch(
    () => props.currentLayerDescription,
    (value) => {
        if (!isDescriptionEditing.value) {
            descriptionDraft.value = value ?? ''
        }
    },
    { immediate: true },
)

async function startDescriptionEditing() {
    descriptionDraft.value = props.currentLayerDescription ?? ''
    isDescriptionEditing.value = true
    await nextTick()
    descriptionInput.value?.textareaRef?.focus()
}

function saveDescription(value: string) {
    emit('update-description', value)
}

function stopDescriptionEditing() {
    saveDescription(descriptionDraft.value)
    isDescriptionEditing.value = false
}

function stopDescriptionEditingShortcut(event: KeyboardEvent) {
    if (!event.ctrlKey && !event.metaKey) return
    event.preventDefault()
    stopDescriptionEditing()
}
</script>

<template>
    <UCard>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-3 items-end">
            <UFormField :label="$t('keymap.layerLabel')" class="min-w-0">
                <div class="flex items-center gap-2">
                    <USelectMenu
                        :model-value="selectedLayerId"
                        :items="layerItems"
                        value-key="value"
                        class="w-full"
                        @update:model-value="
                            (value: string) =>
                                emit('update:selectedLayerId', value)
                        "
                    />
                    <AppTooltip :text="$t('keymap.renameLayerTooltip')">
                        <UButton
                            icon="i-lucide-pencil"
                            size="sm"
                            color="neutral"
                            variant="ghost"
                            :aria-label="
                                $t('keymap.renameLayerAria', {
                                    name: currentLayerName ?? '',
                                })
                            "
                            :disabled="!selectedLayerId"
                            @click="emit('rename')"
                        />
                    </AppTooltip>
                    <AppTooltip :text="$t('keymap.deleteLayerTooltip')">
                        <UButton
                            icon="i-lucide-trash-2"
                            size="sm"
                            color="neutral"
                            variant="ghost"
                            square
                            :aria-label="$t('keymap.delete')"
                            :disabled="!selectedLayerId"
                            @click="$emit('delete')"
                        />
                    </AppTooltip>
                    <UButton
                        icon="i-lucide-copy"
                        size="sm"
                        color="neutral"
                        variant="ghost"
                        :aria-label="
                            $t('keymap.cloneLayerAria', {
                                name: currentLayerName ?? '',
                            })
                        "
                        :disabled="!selectedLayerId"
                        @click="emit('clone')"
                    >
                        {{ $t('common.duplicate') }}
                    </UButton>
                </div>
            </UFormField>
            <div class="flex justify-end items-center gap-2">
                <div class="flex items-center gap-1">
                    <UInput
                        v-model="testInput"
                        :placeholder="$t('keymap.typeTestPlaceholder')"
                        size="sm"
                        class="w-48"
                    />
                    <UButton
                        icon="i-lucide-x"
                        size="sm"
                        color="neutral"
                        variant="ghost"
                        square
                        :disabled="!testInput"
                        :aria-label="$t('common.clear')"
                        @click="testInput = ''"
                    />
                </div>
                <AppTooltip :text="$t('keymap.newLayerTooltip')">
                    <UButton
                        icon="i-lucide-plus"
                        size="sm"
                        @click="$emit('create')"
                    >
                        {{ $t('keymap.newLayer') }}
                    </UButton>
                </AppTooltip>
            </div>
        </div>
        <div class="mt-3">
            <UButton
                v-if="!currentLayerDescription && !isDescriptionEditing"
                size="xs"
                color="neutral"
                variant="soft"
                icon="i-lucide-plus"
                @click="startDescriptionEditing"
            >
                {{ $t('keymap.addDescription') }}
            </UButton>
            <UTextarea
                v-else-if="isDescriptionEditing"
                ref="descriptionInput"
                v-model="descriptionDraft"
                autoresize
                :rows="2"
                :placeholder="$t('rules.layerDescPh')"
                class="w-full"
                @blur="stopDescriptionEditing"
                @keydown.enter="stopDescriptionEditingShortcut"
            />
            <button
                v-else
                type="button"
                class="w-full text-left text-sm text-(--ui-text-muted) p-3 rounded-md bg-(--ui-bg-muted) border border-(--ui-border) transition-colors hover:border-(--ui-border-accented) cursor-text"
                @click="startDescriptionEditing"
            >
                <UIcon
                    name="i-lucide-info"
                    class="w-3.5 h-3.5 mr-1 align-middle"
                />
                {{ currentLayerDescription }}
            </button>
        </div>
    </UCard>
</template>
