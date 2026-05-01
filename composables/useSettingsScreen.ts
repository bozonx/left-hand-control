import type { SettingsScreenState } from "~/composables/settingsScreen/types";
import { useSettingsIssues } from "~/composables/settingsScreen/issues";
import { useSettingsLayoutActions } from "~/composables/settingsScreen/layoutActions";
import { useSettingsMapperControls } from "~/composables/settingsScreen/mapperControls";
import { useSettingsPreferences } from "~/composables/settingsScreen/preferences";

let singleton: SettingsScreenState | null = null;

export function resetSettingsScreenStateForTests() {
  singleton = null;
}

export function useSettingsScreen(): SettingsScreenState {
  if (singleton !== null) return singleton;

  const {
    config,
    settingsDir,
    flush,
    applyPreset,
    markLayoutSavedAs,
    replaceCurrentLayoutSnapshot,
    resetCurrentLayout,
    currentLayoutId,
    isLayoutDirty,
  } = useConfig();
  const library = useLayoutLibrary();
  const mapper = useMapper();
  const platform = usePlatformInfo();
  const theme = useAppTheme();
  const appLocale = useAppLocale();

  const currentLayoutDescription = computed(() => config.value.layoutDescription ?? "");
  const { appearanceItems, localeItems } = useSettingsPreferences(appLocale);
  const { globalBanner, globalIssues, mapperIssues } = useSettingsIssues(platform);
  const layoutActions = useSettingsLayoutActions({
    config,
    currentLayoutId,
    currentLayoutDescription,
    library,
    applyPreset,
    markLayoutSavedAs,
    replaceCurrentLayoutSnapshot,
    resetCurrentLayout,
    flush,
  });
  const mapperControls = useSettingsMapperControls({
    config,
    mapper,
    flush,
  });

  onMounted(async () => {
    await Promise.all([
      mapper.refreshDevices(),
      mapper.refreshStatus(),
      platform.refresh(),
      library.refresh(),
    ]);
  });

  singleton = {
    config,
    settingsDir,
    currentLayoutId,
    currentLayoutDescription,
    isLayoutDirty,
    library,
    mapper,
    platform,
    globalBanner,
    globalIssues,
    mapperIssues,
    theme,
    appLocale,
    appearanceItems,
    localeItems,
    ...layoutActions,
    ...mapperControls,
  };
  return singleton;
}
