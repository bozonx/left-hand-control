/* eslint-disable vue/one-component-per-file */
import { defineComponent, ref } from 'vue'

import { mockNuxtImport, mountSuspended } from '@nuxt/test-utils/runtime'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import CommandsTab from '~/components/CommandsTab.vue'
import { createDefaultConfig } from '~/types/config'

const { useConfigMock, useCommandEditorMock, useListKeyboardNavigationMock } = vi.hoisted(() => ({
  useConfigMock: vi.fn(),
  useCommandEditorMock: vi.fn(),
  useListKeyboardNavigationMock: vi.fn(),
}))

mockNuxtImport('useConfig', () => useConfigMock)
mockNuxtImport('useCommandEditor', () => useCommandEditorMock)
mockNuxtImport('useListKeyboardNavigation', () => useListKeyboardNavigationMock)

const CommandEditorCardStub = defineComponent({
  props: {
    uiKey: { type: String, required: true },
    command: { type: Object, required: true },
    nameInputId: { type: String, required: true },
    usage: { type: Array, default: () => [] },
    idError: { type: String, default: undefined },
    linuxError: { type: String, default: undefined },
    isFirst: { type: Boolean, default: false },
    isLast: { type: Boolean, default: false },
    focusName: { type: Boolean, default: false },
    selected: { type: Boolean, default: false },
  },
  emits: ['select', 'remove', 'moveUp', 'moveDown', 'nameFocused'],
  template: `
    <div data-testid="command-card" :data-ui-key="uiKey">
      <button data-testid="remove-btn" @click="$emit('remove', { uiKey, id: command.id })">remove</button>
      <button data-testid="move-up" @click="$emit('moveUp', uiKey)">up</button>
      <button data-testid="move-down" @click="$emit('moveDown', uiKey)">down</button>
    </div>
  `,
})

const AppTooltipStub = defineComponent({
  props: { text: { type: String, default: '' }, disabled: { type: Boolean, default: false } },
  template: '<div><slot /></div>',
})

const UModalStub = defineComponent({
  props: { open: { type: Boolean, default: false } },
  emits: ['update:open'],
  template: `
    <div v-if="open" data-testid="modal">
      <slot name="body" />
      <slot name="footer" />
    </div>
  `,
})

function makeConfigState(commands: any[] = []) {
  const config = ref(createDefaultConfig())
  config.value.commands = commands
  return { config }
}

function makeEditorApi(commands: any[]) {
  return {
    addCommand: vi.fn(() => {
      const cmd = { id: 'new-cmd', name: 'New command', linux: '' }
      commands.unshift(cmd)
      return cmd
    }),
    removeCommand: vi.fn((uiKey: string) => {
      const idx = commands.findIndex((c) => `cmd-${c.id}` === uiKey)
      if (idx !== -1) commands.splice(idx, 1)
    }),
    moveCommand: vi.fn(),
    uiKeyOf: (c: any) => `cmd-${c.id}`,
    idError: vi.fn().mockReturnValue(undefined),
    linuxError: vi.fn().mockReturnValue(undefined),
    hasErrors: false,
    usage: {} as Record<string, string[]>,
  }
}

describe('CommandsTab', () => {
  beforeEach(() => {
    useConfigMock.mockReset()
    useCommandEditorMock.mockReset()
    useListKeyboardNavigationMock.mockReset()
  })

  it('shows empty state when no commands', async () => {
    const { config } = makeConfigState([])
    useConfigMock.mockReturnValue({ config })
    useCommandEditorMock.mockReturnValue(makeEditorApi([]))
    useListKeyboardNavigationMock.mockReturnValue({
      selectedId: ref(null),
      select: vi.fn(),
      containerRef: ref(null),
    })

    const wrapper = await mountSuspended(CommandsTab, {
      global: {
        stubs: {
          CommandEditorCard: CommandEditorCardStub,
          AppTooltip: AppTooltipStub,
          UModal: UModalStub,
        },
      },
    })

    expect(wrapper.text()).toContain('No commands yet')
    expect(wrapper.findAll('[data-testid="command-card"]')).toHaveLength(0)
  })

  it('renders command cards and wires remove via modal', async () => {
    const commands = [
      { id: 'term', name: 'Terminal', linux: 'kitty' },
      { id: 'browser', name: 'Browser', linux: 'firefox' },
    ]
    const { config } = makeConfigState(commands)
    const editorApi = makeEditorApi(commands)
    useConfigMock.mockReturnValue({ config })
    useCommandEditorMock.mockReturnValue(editorApi)
    useListKeyboardNavigationMock.mockReturnValue({
      selectedId: ref(null),
      select: vi.fn(),
      containerRef: ref(null),
    })

    const wrapper = await mountSuspended(CommandsTab, {
      global: {
        stubs: {
          CommandEditorCard: CommandEditorCardStub,
          AppTooltip: AppTooltipStub,
          UModal: UModalStub,
        },
      },
    })

    const cards = wrapper.findAll('[data-testid="command-card"]')
    expect(cards).toHaveLength(2)

    await cards[0]!.find('[data-testid="remove-btn"]').trigger('click')
    expect(wrapper.find('[data-testid="modal"]').exists()).toBe(true)

    const footerBtns = wrapper.find('[data-testid="modal"]').findAll('button')
    const confirmBtn = footerBtns.find((b) => b.text().includes('Delete'))
    expect(confirmBtn).toBeTruthy()
    await confirmBtn!.trigger('click')

    expect(editorApi.removeCommand).toHaveBeenCalledWith('cmd-term')
    expect(wrapper.find('[data-testid="modal"]').exists()).toBe(false)
  })

  it('cancels remove modal', async () => {
    const commands = [{ id: 'term', name: 'Terminal', linux: 'kitty' }]
    const { config } = makeConfigState(commands)
    const editorApi = makeEditorApi(commands)
    useConfigMock.mockReturnValue({ config })
    useCommandEditorMock.mockReturnValue(editorApi)
    useListKeyboardNavigationMock.mockReturnValue({
      selectedId: ref(null),
      select: vi.fn(),
      containerRef: ref(null),
    })

    const wrapper = await mountSuspended(CommandsTab, {
      global: {
        stubs: {
          CommandEditorCard: CommandEditorCardStub,
          AppTooltip: AppTooltipStub,
          UModal: UModalStub,
        },
      },
    })

    await wrapper.find('[data-testid="remove-btn"]').trigger('click')
    const modal = wrapper.find('[data-testid="modal"]')
    expect(modal.exists()).toBe(true)

    const cancelBtn = modal.findAll('button').find((b) => b.text().includes('Cancel'))
    expect(cancelBtn).toBeTruthy()
    await cancelBtn!.trigger('click')

    expect(editorApi.removeCommand).not.toHaveBeenCalled()
    expect(wrapper.find('[data-testid="modal"]').exists()).toBe(false)
  })

  it('calls addCommand when add button is clicked', async () => {
    const commands: any[] = []
    const { config } = makeConfigState(commands)
    const editorApi = makeEditorApi(commands)
    useConfigMock.mockReturnValue({ config })
    useCommandEditorMock.mockReturnValue(editorApi)
    useListKeyboardNavigationMock.mockReturnValue({
      selectedId: ref(null),
      select: vi.fn(),
      containerRef: ref(null),
    })

    const wrapper = await mountSuspended(CommandsTab, {
      global: {
        stubs: {
          CommandEditorCard: CommandEditorCardStub,
          AppTooltip: AppTooltipStub,
          UModal: UModalStub,
        },
      },
    })

    const addBtn = wrapper.find('button')
    expect(addBtn.text()).toContain('New command')
    await addBtn.trigger('click')
    expect(editorApi.addCommand).toHaveBeenCalledTimes(1)
  })
})
