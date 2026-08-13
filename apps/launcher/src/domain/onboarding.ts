export const ONBOARDING_STORAGE_KEY = "sre.onboarding.v1";
export const stages = [
  { id: "welcome", title: "Welcome to SRE", explanation: "One library, multiple runtimes, and a guided setup that keeps technical details optional." },
  { id: "account", title: "Account Login", explanation: "Browser authentication links your Accord, devices, entitlements, and synchronized achievements." },
  { id: "device", title: "Register this device", explanation: "SRE creates a random UUID and local keypair. No hardware fingerprint is used." },
  { id: "accord", title: "The Great Zelda–Satisfactory Accords", explanation: "The seventeen-article agreement is cooperative, non-destructive, and bounded by real-life human limits." },
  { id: "library", title: "Library Initialization", explanation: "SRE loads the signed-off catalog without scanning arbitrary folders or looking for proprietary content." },
  { id: "runtimes", title: "Runtime Detection", explanation: "Bundled native support is checked locally. External runtimes remain manual, explicit selections." },
  { id: "system", title: "System Check", explanation: "SRE Doctor validates the catalog, Accord, storage, and runtime prerequisites with actionable results." },
  { id: "ready", title: "Ready", explanation: "Choose a game and follow its provider-specific setup checklist." },
] as const;
export type StageId = (typeof stages)[number]["id"];
export interface OnboardingState { version: 1; stage: StageId; accountMode?: "connected" | "offline"; accordAccepted: boolean; deviceId?: string; complete: boolean }
export const initialOnboardingState: OnboardingState = { version: 1, stage: "welcome", accordAccepted: false, complete: false };
export function stageIndex(stage: StageId): number { return stages.findIndex((value) => value.id === stage); }
export function stageProgress(stage: StageId): number { return ((stageIndex(stage) + 1) / stages.length) * 100; }
export function advanceStage(state: OnboardingState): OnboardingState { const index = stageIndex(state.stage); return index < 0 || index >= stages.length - 1 ? state : { ...state, stage: stages[index + 1].id }; }
export function previousStage(state: OnboardingState): OnboardingState { const index = stageIndex(state.stage); return index <= 0 ? state : { ...state, stage: stages[index - 1].id }; }
export function restoreOnboardingState(raw: string | null): OnboardingState { if (!raw) return initialOnboardingState; try { const value = JSON.parse(raw) as Partial<OnboardingState>; return value.version === 1 && stages.some((stage) => stage.id === value.stage) ? { ...initialOnboardingState, ...value, stage: value.stage as StageId } : initialOnboardingState; } catch { return initialOnboardingState; } }
