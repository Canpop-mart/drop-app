import type { Component } from "vue";

export type NavigationItem = {
  prefix: string;
  route: string;
  label: string;
};

export type QuickActionNav = {
  icon: Component;
  notifications?: number;
  action: () => Promise<void>;
};

export type User = {
  id: string;
  username: string;
  admin: boolean;
  displayName: string;
  profilePictureObjectId: string;
};

type UmuState = "Installed" | "NotInstalled" | "NotNeeded";

/**
 * Describes the display session the app is running in.
 * Detected on the Rust side from environment variables and hardware info.
 */
export type SessionType = "desktop" | "gamescope" | "steamDeckDesktop";

export type AppState = {
  status: AppStatus;
  umuState: UmuState;
  sessionType: SessionType;
  user?: User;
};

export type Game = {
  id: string;
  type: "Game" | "Executor" | "Redist" | "Mod";
  mName: string;
  mShortDescription: string;
  mDescription: string;
  mIconObjectId: string;
  mBannerObjectId: string;
  mCoverObjectId: string;
  mImageLibraryObjectIds: string[];
  mImageCarouselObjectIds: string[];
  /** Gamepad support level from Steam (full/partial/none). */
  mControllerSupport?: "Full" | "Partial" | "None";
  /** HowLongToBeat completion times in minutes (best-effort; may be null). */
  mHltbMain?: number | null;
  mHltbMainSides?: number | null;
  mHltbCompletionist?: number | null;
};

export type Collection = {
  id: string;
  name: string;
  isDefault: boolean;
  isTools?: boolean;
  entries: Array<{ gameId: string; game: Game }>;
};

// Physical pad layout override for emulator launches; `null` means detect it.
export type ControllerType = "Xbox" | "PlayStation" | "Nintendo";
export type QualityPreset = "Low" | "Medium" | "High" | "Ultra";
export type MangoHudPreset = "off" | "minimal" | "standard" | "full";
export type AspectRatio = "Standard" | "Wide16_9" | "Wide16_10";

export type GameVersion = {
  userConfiguration: {
    launchTemplate: string;
    overrideProtonPath: string;
    enableUpdates: boolean;
    controllerType: ControllerType | null;
    qualityPreset: QualityPreset | null;
    widescreen: AspectRatio;
    // Backend stores Option<bool>: null = no explicit preference (RetroArch
    // default, which is fullscreen on). The UI collapses null/true into "on".
    fullscreen: boolean | null;
    mangohud: MangoHudPreset | null;
    crtShader: boolean;
    // Which binary inside the install to run, relative to the install dir
    // (e.g. "bin/Game64.exe"). Local to this machine, so the Deck and the
    // desktop can point at different files. null = use the server's command.
    executableOverride: string | null;
  };
  setups: Array<{ platform: string }>;
  launches: Array<{
    platform: string;
    emulator?: {
      gameId: string;
      versionId: string;
      launchId: string;
    };
  }>;
};

export enum AppStatus {
  NotConfigured = "NotConfigured",
  Offline = "Offline",
  SignedOut = "SignedOut",
  SignedIn = "SignedIn",
  SignedInNeedsReauth = "SignedInNeedsReauth",
  ServerUnavailable = "ServerUnavailable",
}

export type EmptyGameStatusEnum =
  | "Remote"
  | "Queued"
  | "Downloading"
  | "Validating"
  | "Updating"
  | "Uninstalling"
  | "Running";

export enum InstalledType {
  PartiallyInstalled = "PartiallyInstalled",
  SetupRequired = "SetupRequired",
  Installed = "Installed",
}

export interface InstalledGameStatusData {
  install_type: { type: InstalledType };
  version_id: string;
  install_dir: string;
  update_available: boolean;
}

export type GameStatus =
  | {
      type: EmptyGameStatusEnum;
    }
  | ({
      type: "Installed";
    } & InstalledGameStatusData);

export type GameStatusEnum = GameStatus["type"];

export type RawGameStatus = [GameStatus | null, GameStatus | null];

export enum DownloadableType {
  Game = "Game",
  Tool = "Tool",
  DLC = "DLC",
  Mod = "Mod",
}

export type DownloadableMetadata = {
  id: string;
  version: string;
  downloadType: DownloadableType;
};

export type Settings = {
  autostart: boolean;
  maxDownloadThreads: number;
  forceOffline: boolean;
  globalMangohud?: MangoHudPreset | null;
  /** Master toggle for cloud save sync. Defaults to true. */
  cloudSavesEnabled?: boolean;
  /** Friendly per-device label shown in the cloud save conflict UI.
   * When null/empty, the backend falls back to the OS hostname. */
  deviceName?: string | null;
  /** Name shown to others in multiplayer (co-op + Archipelago).
   * When null/empty, falls back to the account name. */
  displayName?: string | null;
};
