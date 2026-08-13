import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  DiagnosticCheck,
  OnboardingState,
  TreatyAcceptance,
  ValidationReport,
} from "../domain/onboarding";

export type TreatyMetadata = Omit<TreatyAcceptance, "acceptedAt">;

export interface ImportReport {
  archiveName: string;
  installationId: string;
  importedAtUnixMs: number;
}

export interface LaunchReport {
  processId: number;
  runtimeId: string;
  runtimeVersion: string;
  runtimeProtocol: number;
}

export function isDesktopLauncher(): boolean {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

export async function selectGameData(): Promise<string | null> {
  if (!isDesktopLauncher()) {
    return null;
  }
  const selected = await open({
    title: "Locate Your Zelda Game Data",
    multiple: false,
    directory: false,
    filters: [
      {
        name: "Nintendo 64 game data",
        extensions: ["z64", "n64", "v64"],
      },
      {
        name: "All files",
        extensions: ["*"],
      },
    ],
  });
  return typeof selected === "string" ? selected : null;
}

export async function validateSelectedGameData(path: string): Promise<ValidationReport> {
  if (!isDesktopLauncher()) {
    throw new Error("FICSIT-0006: Native validation is available in the installed desktop launcher.");
  }
  return invoke<ValidationReport>("validate_game_data", { path });
}

export async function getTreatyMetadata(): Promise<TreatyMetadata> {
  if (!isDesktopLauncher()) {
    return {
      treatyId: "FICSIT-TREATY-0001",
      version: "2.0.0",
      sha256: "web-preview-not-an-acceptance-record",
    };
  }
  return invoke<TreatyMetadata>("treaty_metadata");
}

export async function loadNativeOnboardingState(): Promise<OnboardingState | null> {
  if (!isDesktopLauncher()) return null;
  return invoke<OnboardingState | null>("load_onboarding_state");
}

export async function saveNativeOnboardingState(state: OnboardingState): Promise<void> {
  if (!isDesktopLauncher()) return;
  await invoke("save_onboarding_state", { state });
}

export async function importSelectedGameData(
  path: string,
  expectedSha1: string,
  detectedVersion: string,
): Promise<ImportReport> {
  if (!isDesktopLauncher()) {
    throw new Error("FICSIT-0006: Asset import requires the installed desktop launcher.");
  }
  return invoke<ImportReport>("import_game_data", {
    request: { path, expectedSha1, detectedVersion },
  });
}

export async function cancelGameDataImport(): Promise<boolean> {
  if (!isDesktopLauncher()) return false;
  return invoke<boolean>("cancel_game_data_import");
}

export async function launchZelda(state: OnboardingState): Promise<LaunchReport> {
  if (!isDesktopLauncher()) {
    throw new Error("FICSIT-0008: Zelda can only be launched by the installed desktop launcher.");
  }
  return invoke<LaunchReport>("launch_game", {
    request: {
      gameId: "zelda-oot",
      synthetic: state.synthetic,
      entitlementActive: state.entitlementActive,
    },
  });
}

export async function runPreflight(state: OnboardingState): Promise<DiagnosticCheck[]> {
  if (!isDesktopLauncher()) {
    if (!state.synthetic) {
      throw new Error("FICSIT-0009: Native diagnostics require the installed desktop launcher.");
    }
    return browserSyntheticChecks();
  }

  return invoke<DiagnosticCheck[]>("run_preflight", {
    request: {
      synthetic: state.synthetic,
      gameDataValidated: Boolean(state.validation?.integrityValidated),
      assetsImported: state.assetsImported,
      authenticated: Boolean(state.accountName),
      deviceRegistered: state.deviceRegistered,
      entitlementActive: state.entitlementActive,
    },
  });
}

function browserSyntheticChecks(): DiagnosticCheck[] {
  return [
    "Launcher shell",
    "Synthetic game data",
    "Application storage",
    "Treaty acceptance",
    "Synthetic device",
    "Synthetic entitlement",
    "Controller workflow",
    "Graphics workflow",
  ].map((label, index) => ({
    id: `synthetic-${index}`,
    label,
    status: "PASS" as const,
    summary: "Synthetic inspection passed",
    details: "This proves onboarding behavior only and does not authorize or launch Zelda.",
  }));
}
