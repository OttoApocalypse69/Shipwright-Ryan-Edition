import { describe, expect, it } from "vitest";
import { advanceStage, initialOnboardingState, previousStage, restoreOnboardingState, stageProgress, stages } from "./onboarding";

describe("SRE first-run state machine", () => {
  it("uses the release-candidate sequence", () => { expect(stages.map((stage) => stage.id)).toEqual(["welcome", "account", "device", "accord", "library", "runtimes", "system", "ready"]); });
  it("advances and moves back without crossing boundaries", () => { const account = advanceStage(initialOnboardingState); expect(account.stage).toBe("account"); expect(previousStage(account).stage).toBe("welcome"); expect(previousStage(initialOnboardingState)).toBe(initialOnboardingState); });
  it("reports deterministic progress", () => { expect(stageProgress("welcome")).toBe(12.5); expect(stageProgress("ready")).toBe(100); });
  it("restores valid state and rejects corrupt input", () => { expect(restoreOnboardingState(JSON.stringify({ ...initialOnboardingState, stage: "accord" }))).toMatchObject({ stage: "accord" }); expect(restoreOnboardingState("broken")).toEqual(initialOnboardingState); });
});
