import { invoke } from '@tauri-apps/api/core'

export function usePopupBenchmark(windowName: 'emoji' | 'quick') {
    let probePending = false

    async function stage(value: number) {
        await invoke('popup_stage', { window: windowName, stage: value }).catch(
            () => undefined,
        )
    }

    async function ready() {
        const enabled = await invoke<boolean>('benchmark_active').catch(
            () => false,
        )
        if (!enabled) return
        await new Promise<void>((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
        )
        await stage(3)
        while (!document.hasFocus()) {
            await new Promise((resolve) => setTimeout(resolve, 5))
        }
        await stage(4)
        probePending = true
        window.dispatchEvent(
            new KeyboardEvent('keydown', {
                key: 'ArrowDown',
                code: 'ArrowDown',
                bubbles: true,
            }),
        )
    }

    function handleKeydown() {
        if (!probePending) return
        probePending = false
        void stage(5)
    }

    return { ready, handleKeydown }
}
