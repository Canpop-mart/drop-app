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
  type: "Game" | "Executor" | "Redist";
  mName: string;
  mShortDescription: string;
  mDescription: string;
  mIconObjectId: string;
  mBannerObjectId: string;
  mCoverObjectId: string;
  mImageLibraryObjectIds: string[];
  mImageCarouselObjectIds: string[];
};

export type Collection = {
  id: string;
  name: string;
  isDefault: boolean;
  isTools?: boolean;
  entries: Array<{ gameId: string; game: Game }>;
};

export type ControllerType = "Xbox" | "PlayStation" | "Nintendo"; // PlayStation kept for backwards compat, hidden from UI
export type QualityPreset = "Low" | "Medium" | "High" | "Ultra";
export type MangoHudPreset = "off" | "minimal" | "standard" | "full";

export type GameVersion = {
  userConfiguration: {
    launchTemplate: string;
    overrideProtonPath: string;
    enableUpdates: boolean;
    controllerType: ControllerType | null;
    qualityPreset: QualityPreset | null;
    widescreen: boolean;
    mangohud: MangoHudPreset | null;
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
};
