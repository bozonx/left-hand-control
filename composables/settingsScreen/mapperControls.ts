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

const MANUAL_DEVICE_VALUE = "__manual_device_path__";

type DeviceOption = {
  label: string;
  value: string;
};

type DeviceOptionLabel = {
  type: "label";
  label: string;
};

type DeviceOptionGroup = Array<DeviceOption | DeviceOptionLabel>;

export function useSettingsMapperControls({
  config,
  mapper,
  flush,
}: MapperControlOptions) {
  const toast = useToast();
  const { t } = useI18n();

  function optionFor(device: { name: string; path: string }): DeviceOption {
    return {
      label: `${device.name}  —  ${device.path}`,
      value: device.path,
    };
  }

  function groupedDeviceOptions(
    preferred: (device: { is_keyboard: boolean; is_mouse: boolean }) => boolean,
    preferredLabel: string,
    otherLabel: string,
  ): DeviceOptionGroup[] {
    const preferredDevices = mapper.inputDevices.value.filter(preferred);
    const otherDevices = mapper.inputDevices.value.filter((device) => !preferred(device));
    const groups: DeviceOptionGroup[] = [];
    if (preferredDevices.length > 0) {
      groups.push([{ type: "label", label: preferredLabel }, ...preferredDevices.map(optionFor)]);
    }
    if (otherDevices.length > 0) {
      groups.push([{ type: "label", label: otherLabel }, ...otherDevices.map(optionFor)]);
    }
    groups.push([
      { type: "label", label: t("settings.manualDeviceGroup") },
      { label: t("settings.manualDeviceOption"), value: MANUAL_DEVICE_VALUE },
    ]);
    return groups;
  }

  const deviceOptions = computed(() =>
    groupedDeviceOptions(
      (device) => device.is_keyboard,
      t("settings.keyboardLikeDevices"),
      t("settings.otherDevices"),
    ),
  );

  const selectedDevice = computed<string>({
    get: () => config.value.settings.inputDevicePath ?? "",
    set: (value: string) => {
      config.value.settings.inputDevicePath = value;
    },
  });

  const mouseOptions = computed(() =>
    groupedDeviceOptions(
      (device) => device.is_mouse,
      t("settings.mouseLikeDevices"),
      t("settings.otherDevices"),
    ),
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
    MANUAL_DEVICE_VALUE,
  };
}
