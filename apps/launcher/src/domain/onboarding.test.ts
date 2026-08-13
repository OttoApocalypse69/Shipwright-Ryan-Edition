import { describe, expect, it } from "vitest";
import {
  advanceStage,
  initialOnboardingState,
  previousStage,
  restoreOnboardingState,
  stageProgress,
  stages,
} from "./onboarding";

describe("onboarding state machine", () => {
  it("uses the complete Ryan-proof sequence", () => {
    expect(stages.map((stage) => stage.id)).toEqual([
      "welcome",
      "account-login",
      "treaty-review",
      "treaty-acceptance",
      "game-data-selection",
      "game-data-validation",
      "asset-import",
      "device-registration",
      "entitlement-activation",
      "controller-check",
      "graphics-check",
      "system-diagnostics",
      "ready",
    ]);
  });

  it("advances and safely moves back without crossing the boundaries", () => {
    const account = advanceStage(initialOnboardingState);
    expect(account.stage).toBe("account-login");
    expect(previousStage(account).stage).toBe("welcome");
    expect(previousStage(initialOnboardingState)).toBe(initialOnboardingState);

    const ready = { ...initialOnboardingState, stage: "ready" as const };
    expect(advanceStage(ready)).toBe(ready);
  });

  it("reports deterministic progress from the first through final stage", () => {
    expect(stageProgress("welcome")).toBeCloseTo(100 / stages.length);
    expect(stageProgress("ready")).toBe(100);
  });

  it("restores valid checkpoints and rejects corrupt or stale state", () => {
    expect(
      restoreOnboardingState(
        JSON.stringify({ ...initialOnboardingState, stage: "game-data-selection", accountName: "Ryan" }),
      ),
    ).toMatchObject({ stage: "game-data-selection", accountName: "Ryan" });
    expect(restoreOnboardingState("not-json")).toEqual(initialOnboardingState);
    expect(restoreOnboardingState(JSON.stringify({ version: 999, stage: "ready" }))).toEqual(
      initialOnboardingState,
    );
  });
});
