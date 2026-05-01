import type { CapabilityKind, SettingsIssue } from "./types";
import type { CapabilityStatus } from "~/types/platform";

export function useSettingsIssues(platform: ReturnType<typeof usePlatformInfo>) {
  const { t } = useI18n();

  function describeCapabilityIssue(
    kind: CapabilityKind,
    status: CapabilityStatus,
  ): SettingsIssue | null {
    const desktop = platform.info.value?.linux?.desktop ?? platform.info.value?.os ?? "current platform";
    const detail = status.detail?.trim();

    if (kind === "keyInterception" && !status.available) {
      return {
        id: "mapper-key-interception",
        scope: "mapper",
        severity: "error",
        title: t("settings.issues.mapperStartTitle"),
        description: detail || t("settings.issues.mapperStartBody"),
      };
    }

    if (kind === "literalInjection" && !status.available) {
      return {
        id: "mapper-literal-injection",
        scope: "mapper",
        severity: "warning",
        title: t("settings.issues.literalInjectionTitle"),
        description: detail || t("settings.issues.literalInjectionBody"),
      };
    }

    if (kind === "layoutDetection" && !status.supported) {
      return {
        id: "global-layout-detection-unsupported",
        scope: "global",
        severity: "warning",
        title: t("settings.issues.layoutDetectionUnsupportedTitle"),
        description: t("settings.issues.layoutDetectionUnsupportedBody", { desktop }),
      };
    }

    if (kind === "layoutDetection" && !status.available) {
      return {
        id: "global-layout-detection-unavailable",
        scope: "global",
        severity: "warning",
        title: t("settings.issues.layoutDetectionUnavailableTitle"),
        description: detail || t("settings.issues.layoutDetectionUnavailableBody", { desktop }),
      };
    }

    if (kind === "systemActions" && !status.supported) {
      return {
        id: "global-system-actions-unsupported",
        scope: "global",
        severity: "warning",
        title: t("settings.issues.systemActionsUnsupportedTitle"),
        description: t("settings.issues.systemActionsUnsupportedBody", { desktop }),
      };
    }

    if (kind === "systemActions" && !status.available) {
      return {
        id: "global-system-actions-unavailable",
        scope: "global",
        severity: "warning",
        title: t("settings.issues.systemActionsUnavailableTitle"),
        description: detail || t("settings.issues.systemActionsUnavailableBody", { desktop }),
      };
    }

    return null;
  }

  const settingsIssues = computed<SettingsIssue[]>(() => {
    if (platform.error.value) {
      return [
        {
          id: "platform-check-error",
          scope: "global",
          severity: "warning",
          title: t("settings.issues.platformCheckTitle"),
          description: platform.error.value,
        },
      ];
    }

    const capabilities = platform.info.value?.capabilities;
    if (!capabilities) return [];

    return [
      describeCapabilityIssue("keyInterception", capabilities.key_interception),
      describeCapabilityIssue("literalInjection", capabilities.literal_injection),
      describeCapabilityIssue("layoutDetection", capabilities.layout_detection),
      describeCapabilityIssue("systemActions", capabilities.system_actions),
    ].filter((issue): issue is SettingsIssue => issue !== null);
  });

  const globalIssues = computed(() =>
    settingsIssues.value.filter((issue) => issue.scope === "global"),
  );

  const mapperIssues = computed(() =>
    settingsIssues.value.filter((issue) => issue.scope === "mapper"),
  );

  const globalBanner = computed(() => {
    const issues = globalIssues.value;
    if (!issues.length) return null;
    const hasErrors = issues.some((issue) => issue.severity === "error");
    return {
      color: hasErrors ? "error" : "warning",
      icon: hasErrors ? "i-lucide-circle-alert" : "i-lucide-triangle-alert",
      title: hasErrors
        ? t("settings.issues.bannerErrorTitle")
        : t("settings.issues.bannerWarningTitle"),
      issues,
    } as const;
  });

  return {
    globalBanner,
    globalIssues,
    mapperIssues,
  };
}
