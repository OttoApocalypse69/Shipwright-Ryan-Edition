export const ONBOARDING_STORAGE_KEY = "ftep.onboarding.v1";

export const stages = [
  {
    id: "welcome",
    shortTitle: "Welcome",
    title: "Welcome to FTEP",
    explanation: "Diplomatic Zelda operations begin here. The launcher handles the machinery; you handle the clicking.",
    commentary: "FICSIT reminds all beneficiaries that reading terminal output is not a recreational activity.",
  },
  {
    id: "account-login",
    shortTitle: "Account",
    title: "Account Login",
    explanation: "Your account keeps treaty acceptance, devices, entitlements, and achievements together.",
    commentary: "Somehow this needed identity infrastructure.",
  },
  {
    id: "treaty-review",
    shortTitle: "Accords",
    title: "The Great Zelda–Satisfactory Accords",
    explanation: "First the human summary, then all fourteen articles for anyone committed to administrative suffering.",
    commentary: "The legal department is fictional. The scroll bar is unfortunately real.",
  },
  {
    id: "treaty-acceptance",
    shortTitle: "Acceptance",
    title: "User Acceptance",
    explanation: "Confirm that you understand the interpersonal platform agreement.",
    commentary: "Ratification requires one checkbox and a heroic tolerance for nonsense.",
  },
  {
    id: "game-data-selection",
    shortTitle: "Game data",
    title: "Locate Your Zelda Game Data",
    explanation: "Select compatible game data you already possess. FTEP does not provide Nintendo game data.",
    commentary: "Legally supplied bits are the most productive bits.",
  },
  {
    id: "game-data-validation",
    shortTitle: "Validation",
    title: "Game Data Validation",
    explanation: "FTEP checks the selected file locally against Shipwright's supported variants.",
    commentary: "No hashes will be explained unless Advanced Details is opened on purpose.",
  },
  {
    id: "asset-import",
    shortTitle: "Asset import",
    title: "Asset Import",
    explanation: "Validated data is converted into the runtime archive without altering the original file.",
    commentary: "Original material remains untouched under Article I, Subsection Please Do Not Break It.",
  },
  {
    id: "device-registration",
    shortTitle: "Device",
    title: "Device Registration",
    explanation: "FTEP assigns this computer a friendly treaty identity automatically.",
    commentary: "Your gaming apparatus is being recognized with unnecessary ceremony.",
  },
  {
    id: "entitlement-activation",
    shortTitle: "Entitlement",
    title: "Entitlement Activation",
    explanation: "The launcher confirms that the treaty permits Zelda operations on this device.",
    commentary: "Authorization has been routed through the Department of Playing a Video Game.",
  },
  {
    id: "controller-check",
    shortTitle: "Controller",
    title: "Controller Check",
    explanation: "Confirm that a controller is available, or continue with keyboard controls.",
    commentary: "Button integrity is a cornerstone of industrial diplomacy.",
  },
  {
    id: "graphics-check",
    shortTitle: "Graphics",
    title: "Graphics Check",
    explanation: "FTEP verifies that the runtime can create a supported graphics environment.",
    commentary: "Triangles are now undergoing regulatory inspection.",
  },
  {
    id: "system-diagnostics",
    shortTitle: "Diagnostics",
    title: "FICSIT Pre-Flight Inspection",
    explanation: "One final automated inspection checks everything needed for a safe launch.",
    commentary: "Please remain calm while several green rectangles are generated.",
  },
  {
    id: "ready",
    shortTitle: "Ready",
    title: "Ready for Zelda Operations",
    explanation: "Onboarding is complete. No terminal was harmed in the making of this launcher.",
    commentary: "Article I compliance is approaching acceptable levels.",
  },
] as const;

export type StageId = (typeof stages)[number]["id"];

export interface ValidationReport {
  fileName: string;
  fileSize: number;
  readable: boolean;
  formatRecognized: boolean;
  formatName?: string;
  versionSupported: boolean;
  detectedVersion?: string;
  integrityValidated: boolean;
  importPipelineAvailable: boolean;
  sha1: string;
}

export type DiagnosticStatus = "PASS" | "WARNING" | "FAIL";

export interface DiagnosticCheck {
  id: string;
  label: string;
  status: DiagnosticStatus;
  summary: string;
  details: string;
}

export interface TreatyAcceptance {
  treatyId: string;
  version: string;
  sha256: string;
  acceptedAt: string;
}

export interface OnboardingState {
  version: 1;
  stage: StageId;
  accountName?: string;
  treatyAcceptance?: TreatyAcceptance;
  selectedFile?: string;
  selectedFileLabel?: string;
  synthetic: boolean;
  validation?: ValidationReport;
  assetsImported: boolean;
  assetArchiveName?: string;
  assetInstallationId?: string;
  deviceRegistered: boolean;
  entitlementActive: boolean;
  diagnostics?: DiagnosticCheck[];
}

export const initialOnboardingState: OnboardingState = {
  version: 1,
  stage: "welcome",
  synthetic: false,
  assetsImported: false,
  deviceRegistered: false,
  entitlementActive: false,
};

export function stageIndex(stage: StageId): number {
  return stages.findIndex((candidate) => candidate.id === stage);
}

export function stageProgress(stage: StageId): number {
  return ((stageIndex(stage) + 1) / stages.length) * 100;
}

export function advanceStage(state: OnboardingState): OnboardingState {
  const index = stageIndex(state.stage);
  if (index < 0 || index === stages.length - 1) {
    return state;
  }
  return { ...state, stage: stages[index + 1].id };
}

export function previousStage(state: OnboardingState): OnboardingState {
  const index = stageIndex(state.stage);
  if (index <= 0) {
    return state;
  }
  return { ...state, stage: stages[index - 1].id };
}

export function restoreOnboardingState(raw: string | null): OnboardingState {
  if (!raw) {
    return initialOnboardingState;
  }

  try {
    const parsed = JSON.parse(raw) as Partial<OnboardingState>;
    const stageIsValid = stages.some((stage) => stage.id === parsed.stage);
    if (parsed.version !== 1 || !stageIsValid) {
      return initialOnboardingState;
    }
    return {
      ...initialOnboardingState,
      ...parsed,
      stage: parsed.stage as StageId,
    };
  } catch {
    return initialOnboardingState;
  }
}

export function syntheticValidationReport(): ValidationReport {
  return {
    fileName: "FTEP synthetic onboarding fixture",
    fileSize: 0,
    readable: true,
    formatRecognized: true,
    formatName: "Non-playable synthetic fixture",
    versionSupported: true,
    detectedVersion: "Milestone 1.5 demonstration data",
    integrityValidated: true,
    importPipelineAvailable: true,
    sha1: "not-applicable-to-synthetic-data",
  };
}
