import type { AppConfig } from "~/types/config";
import {
  analyzeRules,
  blockingRuleIssues,
  ruleIssueMessageKey,
} from "~/utils/ruleDiagnostics";

interface MapperControlOptions {
  config: Ref<AppConfig>;
  mapper: ReturnType<typeof useMapper>;
  flush: () => Promise<void>;
}

export function useSettingsMapperControls({
  config,
  mapper,
  flush,
}: MapperControlOptions) {
  const toast = useToast();
  const { t } = useI18n();
  const deviceOptions = computed(() =>
    mapper.devices.value.map((device) => ({
      label: `${device.name}  —  ${device.path}`,
      value: device.path,
    })),
  );

  const selectedDevice = computed<string>({
    get: () => config.value.settings.inputDevicePath ?? "",
    set: (value: string) => {
      config.value.settings.inputDevicePath = value;
    },
  });

  const mouseOptions = computed(() =>
    mapper.mice.value.map((device) => ({
      label: `${device.name}  —  ${device.path}`,
      value: device.path,
    })),
  );

  const selectedMouse = computed<string>({
    get: () => config.value.settings.inputMouseDevicePath ?? "",
    set: (value: string) => {
      config.value.settings.inputMouseDevicePath = value;
    },
  });

  async function toggleMapper() {
    try {
      await flush();
      if (mapper.status.value.running) {
        await mapper.stop();
        return;
      }
      if (!selectedDevice.value) return;
      const blockingIssue = blockingRuleIssues(
        analyzeRules(config.value).issues,
      )[0];
      if (blockingIssue) {
        mapper.error.value = t(ruleIssueMessageKey(blockingIssue), {
          trigger: blockingIssue.trigger,
        });
        toast.add({
          title: t("settings.mapperStartFailed"),
          description: mapper.error.value,
          color: "error",
          icon: "i-lucide-circle-alert",
        });
        return;
      }
      await mapper.start(
        selectedDevice.value,
        selectedMouse.value || undefined,
      );
      if (mapper.error.value) {
        toast.add({
          title: t("settings.mapperStartFailed"),
          description: mapper.error.value,
          color: "error",
          icon: "i-lucide-circle-alert",
        });
      }
    } catch (error) {
      mapper.error.value =
        error instanceof Error ? error.message : String(error);
      toast.add({
        title: t("settings.mapperStartFailed"),
        description: mapper.error.value,
        color: "error",
        icon: "i-lucide-circle-alert",
      });
    }
  }

  return {
    deviceOptions,
    selectedDevice,
    mouseOptions,
    selectedMouse,
    toggleMapper,
  };
}
