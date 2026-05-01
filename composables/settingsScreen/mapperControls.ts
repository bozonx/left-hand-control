import type { AppConfig } from "~/types/config";

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
      await mapper.start(
        selectedDevice.value,
        selectedMouse.value || undefined,
      );
    } catch (error) {
      mapper.error.value =
        error instanceof Error ? error.message : String(error);
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
