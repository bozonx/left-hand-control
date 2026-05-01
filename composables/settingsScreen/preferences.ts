import type { AppearancePreference, LocalePreference } from "~/types/config";
import { localeDisplayName } from "~/i18n";

export function useSettingsPreferences(appLocale: ReturnType<typeof useAppLocale>) {
  const { t } = useI18n();

  const appearanceItems = computed<{ label: string; value: AppearancePreference }[]>(() => [
    { label: t("settings.appearanceItems.system"), value: "system" },
    { label: t("settings.appearanceItems.light"), value: "light" },
    { label: t("settings.appearanceItems.dark"), value: "dark" },
  ]);

  const localeItems = computed<{ label: string; value: LocalePreference }[]>(() => {
    const resolvedName = localeDisplayName(appLocale.systemLocale.value);
    return [
      {
        label: t("settings.languageAutoResolved", { resolved: resolvedName }),
        value: "auto",
      },
      ...appLocale.available.map((loc) => ({
        label: localeDisplayName(loc),
        value: loc,
      })),
    ];
  });

  return {
    appearanceItems,
    localeItems,
  };
}
