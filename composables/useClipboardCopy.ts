export function useClipboardCopy() {
  const toast = useToast()
  const { t } = useI18n()

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text)
      toast.add({
        title: t('common.copied'),
        description: text,
        icon: 'i-lucide-copy-check',
        close: true,
      })
    } catch (error) {
      toast.add({
        title: t('common.copy'),
        description: error instanceof Error ? error.message : String(error),
        color: 'error',
        icon: 'i-lucide-circle-alert',
        close: true,
      })
    }
  }

  return { copy }
}
