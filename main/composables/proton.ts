import { invoke } from "@tauri-apps/api/core";

interface ProtonPaths {
  data: Ref<{
    autodiscovered: ProtonPath[];
    custom: ProtonPath[];
    default?: string;
  }>;
  refresh: () => Promise<void>;
}

const protonPaths = useState<ProtonPaths["data"]["value"]>(
  "proton_paths",
  undefined,
);

export const useProtonPaths = async (): Promise<ProtonPaths> => {
  const refresh = async () => {
    try {
      protonPaths.value = await invoke("fetch_proton_paths");
    } catch (e) {
      console.error("Failed to fetch Proton paths:", e);
      throw createError({
        statusCode: 500,
        statusMessage: "Failed to load Proton compatibility data. Please check your Proton installation.",
        fatal: false,
      });
    }
  };
  if (protonPaths.value)
    return {
      data: protonPaths,
      refresh,
    };

  await refresh();
  return {
    data: protonPaths,
    refresh,
  };
};
