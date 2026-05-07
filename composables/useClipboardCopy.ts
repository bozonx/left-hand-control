export function useClipboardCopy() {
  const toast = useToast()
  const { t } = useI18n()

  async function copy(text: string) {
    const description = text.length > 80 ? `${text.slice(0, 77)}...` : text
    try {
      await navigator.clipboard.writeText(text)
      toast.add({
        title: t('common.copied'),
        description,
        icon: 'i-lucide-copy-check',
        close: true,
      })
    } catch (error) {
      toast.add({
        title: t('common.copyFailed'),
        description: error instanceof Error ? error.message : String(error),
        color: 'error',
        icon: 'i-lucide-circle-alert',
        close: true,
      })
    }
  }

  return { copy }
}
