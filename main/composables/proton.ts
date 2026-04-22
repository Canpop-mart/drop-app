import { invoke } from "@tauri-apps/api/core";
import { platform } from "@tauri-apps/plugin-os";

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

// Empty placeholder used on Windows/macOS where `fetch_proton_paths` is
// `#[cfg(target_os = "linux")]` and therefore absent. Prevents callers
// from seeing a thrown createError on a platform that never should have
// asked in the first place.
const EMPTY_PROTON_PATHS = {
  autodiscovered: [] as ProtonPath[],
  custom: [] as ProtonPath[],
  default: undefined as string | undefined,
};

export const useProtonPaths = async (): Promise<ProtonPaths> => {
  const onLinux = platform() === "linux";

  const refresh = async () => {
    if (!onLinux) {
      protonPaths.value = EMPTY_PROTON_PATHS;
      return;
    }
    try {
      protonPaths.value = await invoke("fetch_proton_paths");
    } catch (e) {
      console.error("Failed to fetch Proton paths:", e);
      throw createError({
        statusCode: 500,
        statusMessage:
          "Failed to load Proton compatibility data. Please check your Proton installation.",
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
