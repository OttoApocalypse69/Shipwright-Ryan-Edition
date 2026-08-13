import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { OnboardingState } from "../domain/onboarding";
import fallbackCatalog from "../../../../packages/game-catalog/catalog.v1.json";

export interface CatalogGame { id: string; franchise: string; title: string; description: string; cover: { kind: string; titleMark: string; accentColor: string; backgroundColor: string }; variants: Array<{ id: string; originalPlatform: string; preferredRuntime?: string | null; runtimeCandidates: Array<{ runtimeId: string; compatibility: string }>; sourceRequirements: Array<{ id: string; kind: string; description: string; required: boolean }> }>; preferredVariant: string; achievementNamespace: string }
export interface GameCatalog { schemaVersion: number; games: CatalogGame[] }
export interface LibraryInstallation { installationId: string; gameId: string; variantId: string; runtimeId: string; gameSource: string; runtimeExecutable?: string; status: "NEEDS_SETUP" | "READY" | "ERROR"; configuredAtUnixMs: number; lastError?: string }
export interface LibraryState { installations: Record<string, LibraryInstallation> }
export interface DeviceIdentity { deviceId: string; publicKey: string }
export interface DoctorCheck { id: string; status: "PASS" | "WARNING" | "FAIL"; summary: string; remediation?: string }
export interface DoctorReport { schemaVersion: number; product: string; generatedAtUnixMs: number; platform: string; checks: DoctorCheck[] }
export interface OverlayNotification { id: string; kind: string; title: string; message: string; createdAtUnixMs: number; displayMs: number }
export interface RuntimeSession { sessionId: string; gameId: string; variantId: string; runtimeId: string; state: string; requestedAtUnixMs: number; startedAtUnixMs?: number; playableAtUnixMs?: number; endedAtUnixMs?: number; durationMs?: number; exitCode?: number; launchResult: string }
export interface SessionHistory { sessions: RuntimeSession[] }
export interface ValidationReport { fileName: string; fileSize: number; versionSupported: boolean; detectedVersion?: string; integrityValidated: boolean; importPipelineAvailable: boolean; sha1: string }
export interface ImportReport { archiveName: string; installationId: string; importedAtUnixMs: number }

export function isDesktopLauncher(): boolean { return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__); }
export async function getCatalog(): Promise<GameCatalog> { return isDesktopLauncher() ? invoke<GameCatalog>("game_catalog") : fallbackCatalog as GameCatalog; }
export async function loadLibrary(): Promise<LibraryState> { return isDesktopLauncher() ? invoke<LibraryState>("load_library") : { installations: {} }; }
export async function loadOnboarding(): Promise<OnboardingState | null> { return isDesktopLauncher() ? invoke<OnboardingState | null>("load_onboarding_state") : null; }
export async function saveOnboarding(state: OnboardingState): Promise<void> { if (isDesktopLauncher()) await invoke("save_onboarding_state", { state }); }
export async function getDeviceIdentity(): Promise<DeviceIdentity> { return isDesktopLauncher() ? invoke<DeviceIdentity>("device_identity") : { deviceId: "browser-preview-device", publicKey: "preview-only" }; }
export async function runDoctor(): Promise<DoctorReport> { return isDesktopLauncher() ? invoke<DoctorReport>("run_sre_doctor", { runtimeExecutable: null }) : { schemaVersion: 1, product: "SRE", generatedAtUnixMs: Date.now(), platform: "browser-preview", checks: [{ id: "desktop", status: "WARNING", summary: "Native checks require the installed SRE desktop app." }] }; }
export async function exportDiagnostics(): Promise<void> { if (!isDesktopLauncher()) throw new Error("Install SRE to export native diagnostics."); const destination = await save({ title: "Export sanitized SRE diagnostics", defaultPath: "sre-diagnostics.json", filters: [{ name: "JSON", extensions: ["json"] }] }); if (destination) await invoke("export_diagnostics", { destination }); }
export async function chooseGameSource(directory: boolean): Promise<string | null> { if (!isDesktopLauncher()) return null; const selected = await open({ title: directory ? "Choose game installation directory" : "Choose game file", directory, multiple: false }); return typeof selected === "string" ? selected : null; }
export async function chooseRuntime(): Promise<string | null> { if (!isDesktopLauncher()) return null; const selected = await open({ title: "Choose external runtime executable", directory: false, multiple: false, filters: [{ name: "Windows application", extensions: ["exe"] }] }); return typeof selected === "string" ? selected : null; }
export async function registerGame(request: { gameId: string; variantId: string; runtimeId: string; gameSource: string; runtimeExecutable?: string; switchImplementationId?: string }): Promise<LibraryInstallation> { if (!isDesktopLauncher()) throw new Error("Game registration requires the installed SRE desktop app."); return invoke<LibraryInstallation>("register_game", { request }); }
export async function entitlementStatus(): Promise<string> { if (!isDesktopLauncher()) throw new Error("Signed entitlement verification requires SRE desktop."); return invoke<string>("entitlement_status"); }
export async function connectFtep(): Promise<void> { if (!isDesktopLauncher()) { window.open(`${import.meta.env.VITE_FTEP_WEB_URL ?? "http://localhost:3000"}/dashboard`, "_blank", "noopener,noreferrer"); throw new Error("The signed callback can only complete in SRE desktop."); } const url = await invoke<string>("begin_ftep_connection", { request: { webBaseUrl: import.meta.env.VITE_FTEP_WEB_URL ?? "http://localhost:3000" } }); await openUrl(url); for (let attempt = 0; attempt < 300; attempt += 1) { await new Promise((resolve) => setTimeout(resolve, 1_000)); if (await entitlementStatus().then(() => true).catch(() => false)) return; } throw new Error("FTEP connection timed out. Return to SRE and try again."); }
export async function loadSessions(): Promise<SessionHistory> { return isDesktopLauncher() ? invoke<SessionHistory>("session_history") : { sessions: [] }; }
export async function achievementCount(): Promise<number> { return isDesktopLauncher() ? invoke<number>("achievement_count") : 0; }
export async function launchRegisteredGame(installationId: string): Promise<{ sessionId: string; processId?: number; launchResult: string }> { if (!isDesktopLauncher()) throw new Error("Launching requires SRE desktop."); return invoke("launch_registered_game", { request: { installationId } }); }
export async function chooseOotData(): Promise<string | null> { if (!isDesktopLauncher()) return null; const selected = await open({ title: "Choose legally acquired Ocarina of Time game data", directory: false, multiple: false, filters: [{ name: "Nintendo 64 game data", extensions: ["z64", "n64", "v64"] }] }); return typeof selected === "string" ? selected : null; }
export async function validateOotData(path: string): Promise<ValidationReport> { return invoke<ValidationReport>("validate_game_data", { path }); }
export async function importOotData(path: string, report: ValidationReport): Promise<ImportReport> { return invoke<ImportReport>("import_game_data", { request: { path, expectedSha1: report.sha1, detectedVersion: report.detectedVersion ?? "unknown" } }); }
export async function popOverlay(): Promise<OverlayNotification | null> { return isDesktopLauncher() ? invoke<OverlayNotification | null>("pop_overlay") : null; }
export async function hideOverlay(): Promise<void> { if (isDesktopLauncher()) await invoke("hide_overlay"); }
