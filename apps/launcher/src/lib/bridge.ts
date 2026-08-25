import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { OnboardingState } from "../domain/onboarding";
import fallbackCatalog from "../../../../packages/game-catalog/catalog.v1.json";

export interface CatalogGame { id: string; franchise: string; title: string; description: string; cover: { kind: string; titleMark: string; accentColor: string; backgroundColor: string; bannerUrl?: string }; variants: Array<{ id: string; originalPlatform: string; preferredRuntime?: string | null; runtimeCandidates: Array<{ runtimeId: string; compatibility: string }>; sourceRequirements: Array<{ id: string; kind: string; description: string; required: boolean }> }>; preferredVariant: string; achievementNamespace: string }
export interface GameCatalog { schemaVersion: number; games: CatalogGame[] }
export interface LibraryInstallation { installationId: string; gameId: string; variantId: string; runtimeId: string; gameSource: string; runtimeExecutable?: string; status: "NEEDS_SETUP" | "READY" | "ERROR"; configuredAtUnixMs: number; lastError?: string }
export interface LibraryState { installations: Record<string, LibraryInstallation> }
export interface DeviceIdentity { deviceId: string; publicKey: string }
export interface DoctorCheck { id: string; status: "PASS" | "WARNING" | "FAIL"; summary: string; remediation?: string }
export interface DoctorReport { schemaVersion: number; product: string; generatedAtUnixMs: number; platform: string; checks: DoctorCheck[] }
export interface OverlayNotification { id: string; kind: string; title: string; message: string; createdAtUnixMs: number; displayMs: number }
export interface RuntimeSession { sessionId: string; gameId: string; variantId: string; runtimeId: string; state: string; processId?: number; requestedAtUnixMs: number; startedAtUnixMs?: number; playableAtUnixMs?: number; endedAtUnixMs?: number; durationMs?: number; exitCode?: number; launchResult: string }
export interface SessionHistory { sessions: RuntimeSession[] }
export interface ValidationReport { fileName: string; fileSize: number; versionSupported: boolean; detectedVersion?: string; integrityValidated: boolean; importPipelineAvailable: boolean; sha1: string }
export interface ImportReport { archiveName: string; installationId: string; importedAtUnixMs: number }
export interface EmulatorInfo { id: string; name: string; platform: string; version: string; status: "READY" | "MISSING"; managed: boolean; executablePath?: string; runtimeDirectory?: string; settingsDirectory: string; description: string }

const browserPreviewFtepUrl = import.meta.env.VITE_FTEP_WEB_URL ?? "http://localhost:3000";

export function isDesktopLauncher(): boolean { return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__); }
export async function getCatalog(): Promise<GameCatalog> { return isDesktopLauncher() ? invoke<GameCatalog>("game_catalog") : fallbackCatalog as GameCatalog; }
export async function loadLibrary(): Promise<LibraryState> { return isDesktopLauncher() ? invoke<LibraryState>("load_library") : { installations: {} }; }
export async function loadOnboarding(): Promise<OnboardingState | null> { return isDesktopLauncher() ? invoke<OnboardingState | null>("load_onboarding_state") : null; }
export async function saveOnboarding(state: OnboardingState): Promise<void> { if (isDesktopLauncher()) await invoke("save_onboarding_state", { state }); }
export async function getDeviceIdentity(): Promise<DeviceIdentity> { return isDesktopLauncher() ? invoke<DeviceIdentity>("device_identity") : { deviceId: "browser-preview-device", publicKey: "preview-only" }; }
export async function runDoctor(): Promise<DoctorReport> { return isDesktopLauncher() ? invoke<DoctorReport>("run_sre_doctor", { runtimeExecutable: null }) : { schemaVersion: 1, product: "SRE", generatedAtUnixMs: Date.now(), platform: "browser-preview", checks: [{ id: "desktop", status: "WARNING", summary: "Native checks require the installed SRE desktop app." }] }; }
export async function exportDiagnostics(): Promise<void> { if (!isDesktopLauncher()) throw new Error("Install SRE to export native diagnostics."); const destination = await save({ title: "Export sanitized SRE diagnostics", defaultPath: "sre-diagnostics.json", filters: [{ name: "JSON", extensions: ["json"] }] }); if (destination) await invoke("export_diagnostics", { destination }); }
export async function chooseGameSource(directory: boolean): Promise<string | null> { if (!isDesktopLauncher()) return null; const selected = await open({ title: directory ? "Choose game installation directory" : "Choose game file", directory, multiple: false }); return typeof selected === "string" ? selected : null; }
export async function chooseWiiUData(): Promise<string | null> { if (!isDesktopLauncher()) return null; const selected = await open({ title: "Choose legally acquired Wii U game data", directory: false, multiple: false, filters: [{ name: "Wii U game images", extensions: ["wua", "wux", "wud", "rpx"] }] }); return typeof selected === "string" ? selected : null; }
export async function chooseWiiData(): Promise<string | null> { if (!isDesktopLauncher()) return null; const selected = await open({ title: "Choose legally acquired Wii game data", directory: false, multiple: false, filters: [{ name: "Wii game images", extensions: ["iso", "wbfs", "rvz", "gcz", "wia", "ciso"] }] }); return typeof selected === "string" ? selected : null; }
export async function chooseSwitchData(): Promise<string | null> { if (!isDesktopLauncher()) return null; const selected = await open({ title: "Choose legally acquired Switch game data", directory: false, multiple: false, filters: [{ name: "Switch game images", extensions: ["nsp", "xci", "nca", "nsz", "xcz"] }] }); return typeof selected === "string" ? selected : null; }
export async function chooseRuntime(): Promise<string | null> { if (!isDesktopLauncher()) return null; const selected = await open({ title: "Choose external runtime executable", directory: false, multiple: false, filters: [{ name: "Windows application", extensions: ["exe"] }] }); return typeof selected === "string" ? selected : null; }
export async function registerGame(request: { gameId: string; variantId: string; runtimeId: string; gameSource: string; runtimeExecutable?: string; switchImplementationId?: string }): Promise<LibraryInstallation> { if (!isDesktopLauncher()) throw new Error("Game registration requires the installed SRE desktop app."); return invoke<LibraryInstallation>("register_game", { request }); }
export async function registerImportedShipwrightGame(): Promise<LibraryInstallation> { if (!isDesktopLauncher()) throw new Error("Shipwright registration requires the installed SRE desktop app."); return invoke<LibraryInstallation>("register_imported_shipwright_game"); }
export async function entitlementStatus(): Promise<string> { if (!isDesktopLauncher()) throw new Error("Signed entitlement verification requires SRE desktop."); return invoke<string>("entitlement_status"); }
export async function ftepWebUrl(): Promise<string> { return isDesktopLauncher() ? invoke<string>("ftep_web_url") : browserPreviewFtepUrl; }
type FtepConnectionStatus = { state: "idle" | "pending" | "connected" | "failed"; error?: string };

async function waitForFtepConnection(): Promise<void> {
  for (let attempt = 0; attempt < 300; attempt += 1) {
    const status = await invoke<FtepConnectionStatus>("ftep_connection_status");
    if (status.state === "connected") return;
    if (status.state === "failed") throw new Error(status.error ?? "FTEP connection failed.");
    await new Promise((resolve) => setTimeout(resolve, 1_000));
  }
  throw new Error("FTEP connection timed out. Return to SRE and try again.");
}

export async function connectFtep(): Promise<void> {
  if (!isDesktopLauncher()) {
    window.open(`${browserPreviewFtepUrl}/dashboard`, "_blank", "noopener,noreferrer");
    throw new Error("The signed callback can only complete in SRE desktop.");
  }
  for (let attempt = 0; attempt < 2; attempt += 1) {
    const url = await invoke<string>("begin_ftep_connection", { request: {} });
    await openUrl(url);
    try {
      await waitForFtepConnection();
      return;
    } catch (error) {
      if (attempt === 0 && error instanceof Error && error.message.includes("DEVICE_OWNERSHIP_CONFLICT")) {
        await invoke("rotate_device_identity");
        continue;
      }
      throw error;
    }
  }
}
export async function loadSessions(): Promise<SessionHistory> { return isDesktopLauncher() ? invoke<SessionHistory>("session_history") : { sessions: [] }; }
export async function achievementCount(): Promise<number> { return isDesktopLauncher() ? invoke<number>("achievement_count") : 0; }
export async function launchRegisteredGame(installationId: string): Promise<{ sessionId: string; processId?: number; launchResult: string }> { if (!isDesktopLauncher()) throw new Error("Launching requires SRE desktop."); return invoke("launch_registered_game", { request: { installationId } }); }
export async function cancelRegisteredGameLaunch(): Promise<void> { if (!isDesktopLauncher()) throw new Error("Cancelling requires SRE desktop."); await invoke("cancel_registered_game_launch"); }
export async function stopRunningGame(sessionId: string): Promise<void> { if (!isDesktopLauncher()) throw new Error("Stopping a game requires SRE desktop."); await invoke("stop_running_game", { request: { sessionId } }); }
export async function activeRunningGameSessions(): Promise<string[]> { return isDesktopLauncher() ? invoke<string[]>("active_running_game_sessions") : []; }
export async function chooseShipwrightData(gameTitle: string): Promise<string | null> { if (!isDesktopLauncher()) return null; const selected = await open({ title: `Choose legally acquired ${gameTitle} game data`, directory: false, multiple: false, filters: [{ name: "Nintendo 64 game data", extensions: ["z64", "n64", "v64"] }] }); return typeof selected === "string" ? selected : null; }
export async function validateShipwrightData(path: string): Promise<ValidationReport> { return invoke<ValidationReport>("validate_game_data", { path }); }
export async function importShipwrightData(path: string, report: ValidationReport): Promise<ImportReport> { return invoke<ImportReport>("import_game_data", { request: { path, expectedSha1: report.sha1, detectedVersion: report.detectedVersion ?? "unknown" } }); }
export async function popOverlay(): Promise<OverlayNotification | null> { return isDesktopLauncher() ? invoke<OverlayNotification | null>("pop_overlay") : null; }
export async function hideOverlay(): Promise<void> { if (isDesktopLauncher()) await invoke("hide_overlay"); }
export async function getEmulatorInventory(): Promise<EmulatorInfo[]> { return isDesktopLauncher() ? invoke<EmulatorInfo[]>("emulator_inventory") : []; }
export async function openEmulator(runtimeId: string): Promise<void> { if (!isDesktopLauncher()) throw new Error("Opening emulators requires the installed SRE desktop app."); await invoke("open_emulator", { request: { runtimeId } }); }
export async function openEmulatorSettings(runtimeId: string): Promise<void> { if (!isDesktopLauncher()) throw new Error("Opening emulator settings requires the installed SRE desktop app."); await invoke("open_emulator_settings", { request: { runtimeId } }); }
export async function openEmulatorRuntimeFolder(runtimeId: string): Promise<void> { if (!isDesktopLauncher()) throw new Error("Opening emulator folders requires the installed SRE desktop app."); await invoke("open_emulator_runtime_folder", { request: { runtimeId } }); }
