import { useEffect, useMemo, useState } from "react";
import treatyDocument from "../../../packages/treaty/FICSIT-TREATY-0001.v2.json";
import {
  ONBOARDING_STORAGE_KEY,
  advanceStage,
  initialOnboardingState,
  previousStage,
  restoreOnboardingState,
  stageIndex,
  stageProgress,
  stages,
  syntheticValidationReport,
  type DiagnosticCheck,
  type OnboardingState,
  type ValidationReport,
} from "./domain/onboarding";
import {
  cancelGameDataImport,
  getTreatyMetadata,
  importSelectedGameData,
  isDesktopLauncher,
  launchZelda,
  loadNativeOnboardingState,
  runPreflight,
  saveNativeOnboardingState,
  selectGameData,
  validateSelectedGameData,
  type LaunchReport,
  type TreatyMetadata,
} from "./lib/bridge";

interface TreatyArticle {
  number: number;
  title: string;
  paragraphs: string[];
}

const treaty = treatyDocument as typeof treatyDocument & { articles: TreatyArticle[] };

const validationRows: Array<{
  key: keyof ValidationReport;
  label: string;
}> = [
  { key: "readable", label: "File readable" },
  { key: "formatRecognized", label: "Format recognized" },
  { key: "versionSupported", label: "Version supported" },
  { key: "integrityValidated", label: "Integrity validated" },
  { key: "importPipelineAvailable", label: "Import pipeline available" },
];

function readInitialState(): OnboardingState {
  if (typeof window === "undefined") {
    return initialOnboardingState;
  }
  if (isDesktopLauncher()) {
    return initialOnboardingState;
  }
  return restoreOnboardingState(window.localStorage.getItem(ONBOARDING_STORAGE_KEY));
}

function fileLabel(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? "Selected game data";
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "Synthetic fixture";
  const units = ["B", "KiB", "MiB", "GiB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function StatusPill({ status }: { status: "PASS" | "WARNING" | "FAIL" }) {
  return <span className={`status-pill status-${status.toLowerCase()}`}>{status}</span>;
}

function ValidationResults({ report }: { report: ValidationReport }) {
  return (
    <div className="inspection-list" aria-label="Game-data inspection results">
      {validationRows.map(({ key, label }) => {
        const passed = Boolean(report[key]);
        return (
          <div className="inspection-row" key={key}>
            <span className={`inspection-icon ${passed ? "passed" : "failed"}`} aria-hidden="true">
              {passed ? "✓" : "×"}
            </span>
            <span>{label}</span>
            <strong>{passed ? "READY" : "NOT READY"}</strong>
          </div>
        );
      })}
    </div>
  );
}

function Diagnostics({ checks }: { checks: DiagnosticCheck[] }) {
  return (
    <div className="diagnostic-list" aria-label="Pre-flight diagnostic results">
      {checks.map((check) => (
        <details className="diagnostic-row" key={check.id}>
          <summary>
            <span>{check.label}</span>
            <span className="diagnostic-summary">{check.summary}</span>
            <StatusPill status={check.status} />
          </summary>
          <p>{check.details}</p>
        </details>
      ))}
    </div>
  );
}

function App() {
  const desktop = isDesktopLauncher();
  const [state, setState] = useState<OnboardingState>(readInitialState);
  const [treatyMetadata, setTreatyMetadata] = useState<TreatyMetadata>();
  const [acceptanceChecked, setAcceptanceChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [showCompletion, setShowCompletion] = useState(false);
  const [launchReport, setLaunchReport] = useState<LaunchReport>();
  const [storageReady, setStorageReady] = useState(!desktop);

  const currentStage = useMemo(
    () => stages.find((stage) => stage.id === state.stage) ?? stages[0],
    [state.stage],
  );
  const currentIndex = stageIndex(state.stage);
  useEffect(() => {
    if (!desktop) return;
    loadNativeOnboardingState()
      .then((saved) => {
        if (saved) setState(saved);
      })
      .catch((reason: unknown) => setError(String(reason)))
      .finally(() => setStorageReady(true));
  }, [desktop]);

  useEffect(() => {
    if (!storageReady) return;
    if (desktop) {
      saveNativeOnboardingState(state).catch((reason: unknown) => setError(String(reason)));
    } else {
      window.localStorage.setItem(ONBOARDING_STORAGE_KEY, JSON.stringify(state));
    }
  }, [desktop, state, storageReady]);

  useEffect(() => {
    getTreatyMetadata().then(setTreatyMetadata).catch((reason: unknown) => {
      setError(String(reason));
    });
  }, []);

  function patchState(patch: Partial<OnboardingState>) {
    setState((previous) => ({ ...previous, ...patch }));
  }

  function goBack() {
    setError(undefined);
    setState(previousStage);
  }

  function goForward() {
    setError(undefined);
    setState(advanceStage);
  }

  async function chooseGameData() {
    setError(undefined);
    setBusy(true);
    try {
      const path = await selectGameData();
      if (path) {
        patchState({
          selectedFile: path,
          selectedFileLabel: fileLabel(path),
          synthetic: false,
          validation: undefined,
          assetsImported: false,
          diagnostics: undefined,
        });
      } else if (!desktop) {
        setError("FICSIT-0006: The native file picker is available in the desktop launcher. Use the synthetic path for browser development.");
      }
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  function chooseSyntheticData() {
    setError(undefined);
    patchState({
      selectedFile: "synthetic://ftep-milestone-1.5",
      selectedFileLabel: "FTEP synthetic onboarding fixture",
      synthetic: true,
      validation: undefined,
      assetsImported: false,
      diagnostics: undefined,
    });
  }

  async function inspectGameData() {
    if (!state.selectedFile) return;
    setError(undefined);
    setBusy(true);
    try {
      const report = state.synthetic
        ? syntheticValidationReport()
        : await validateSelectedGameData(state.selectedFile);
      patchState({ validation: report });
      if (!report.versionSupported) {
        setError("FICSIT-0007: The supplied file does not match a currently supported game-data variant.");
      }
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function importAssets() {
    if (!state.selectedFile || !state.validation) return;
    setError(undefined);
    if (state.synthetic) {
      patchState({
        assetsImported: true,
        assetArchiveName: "synthetic-non-playable.fixture",
        assetInstallationId: "synthetic",
      });
      return;
    }

    setBusy(true);
    try {
      const report = await importSelectedGameData(
        state.selectedFile,
        state.validation.sha1,
        state.validation.detectedVersion ?? "Unknown supported version",
      );
      patchState({
        assetsImported: true,
        assetArchiveName: report.archiveName,
        assetInstallationId: report.installationId,
        diagnostics: undefined,
      });
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function cancelImport() {
    const cancellationRequested = await cancelGameDataImport();
    if (cancellationRequested) {
      setError("Asset-import cancellation requested. The staged work will be removed safely.");
    }
  }

  async function inspectSystem() {
    setError(undefined);
    setBusy(true);
    try {
      patchState({ diagnostics: await runPreflight(state) });
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function startZelda() {
    setError(undefined);
    setBusy(true);
    try {
      const report = await launchZelda(state);
      setLaunchReport(report);
      setShowCompletion(true);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function copyDiagnostic() {
    if (!error) return;
    await navigator.clipboard?.writeText(error);
  }

  function resetDemo() {
    window.localStorage.removeItem(ONBOARDING_STORAGE_KEY);
    setState(initialOnboardingState);
    setError(undefined);
    setAcceptanceChecked(false);
    setShowCompletion(false);
    setLaunchReport(undefined);
  }

  const diagnosticsReady = Boolean(
    state.diagnostics?.length && !state.diagnostics.some((check) => check.status === "FAIL"),
  );

  const primary = (() => {
    switch (state.stage) {
      case "welcome":
        return { label: "Begin Onboarding", action: goForward, disabled: false };
      case "account-login":
        return state.accountName
          ? { label: "Continue", action: goForward, disabled: false }
          : {
              label: desktop ? "Use Local Milestone Account" : "Use Demonstration Account",
              action: () =>
                patchState({
                  accountName: desktop ? "Ryan — Local Milestone Account" : "Ryan — Demonstration Account",
                }),
              disabled: false,
            };
      case "treaty-review":
        return { label: "Continue", action: goForward, disabled: false };
      case "treaty-acceptance":
        return {
          label: "Ratify Treaty",
          action: () => {
            if (!treatyMetadata) return;
            patchState({
              treatyAcceptance: {
                ...treatyMetadata,
                acceptedAt: new Date().toISOString(),
              },
              stage: "game-data-selection",
            });
          },
          disabled: !acceptanceChecked || !treatyMetadata,
        };
      case "game-data-selection":
        return { label: "Continue", action: goForward, disabled: !state.selectedFile };
      case "game-data-validation":
        return state.validation?.versionSupported
          ? { label: "Continue", action: goForward, disabled: false }
          : { label: "Inspect Game Data", action: inspectGameData, disabled: !state.selectedFile };
      case "asset-import":
        if (state.assetsImported) {
          return { label: "Continue", action: goForward, disabled: false };
        }
        return {
          label: state.synthetic ? "Prepare Synthetic Assets" : "Import Assets",
          action: importAssets,
          disabled: !state.validation?.importPipelineAvailable,
        };
      case "device-registration":
        return state.deviceRegistered
          ? { label: "Continue", action: goForward, disabled: false }
          : {
              label: state.synthetic ? "Register Demonstration Device" : "Register This Windows Device",
              action: () => patchState({ deviceRegistered: true }),
              disabled: false,
            };
      case "entitlement-activation":
        return state.entitlementActive
          ? { label: "Continue", action: goForward, disabled: false }
          : {
              label: state.synthetic ? "Activate Demonstration Entitlement" : "Activate Local Milestone Entitlement",
              action: () => patchState({ entitlementActive: true }),
              disabled: !state.deviceRegistered || !state.treatyAcceptance,
            };
      case "controller-check":
      case "graphics-check":
        return {
          label: state.synthetic ? "Continue Synthetic Check" : "Continue to Native Pre-Flight",
          action: goForward,
          disabled: false,
        };
      case "system-diagnostics":
        return diagnosticsReady
          ? { label: "Continue", action: goForward, disabled: false }
          : { label: "Run Pre-Flight Inspection", action: inspectSystem, disabled: false };
      case "ready":
        return {
          label: state.synthetic ? "Complete Demonstration" : "Play Zelda",
          action: state.synthetic ? () => setShowCompletion(true) : startZelda,
          disabled: !diagnosticsReady,
        };
    }
  })();

  return (
    <main className="app-shell">
      <aside className="stage-rail" aria-label="Onboarding progress">
        <div className="brand-lockup">
          <div className="brand-mark" aria-hidden="true">
            FT
          </div>
          <div>
            <strong>FTEP</strong>
            <span>Compliance Launcher</span>
          </div>
        </div>

        <div className="rail-caption">Diplomatic onboarding</div>
        <ol className="stage-list">
          {stages.map((stage, index) => {
            const status = index < currentIndex ? "complete" : index === currentIndex ? "current" : "upcoming";
            return (
              <li className={`stage-item ${status}`} key={stage.id} aria-current={status === "current" ? "step" : undefined}>
                <span className="stage-number">{status === "complete" ? "✓" : String(index + 1).padStart(2, "0")}</span>
                <span>{stage.shortTitle}</span>
              </li>
            );
          })}
        </ol>

        <div className="rail-footer">
          <span className={`environment-dot ${desktop ? "native" : "browser"}`} />
          {desktop ? "Windows desktop boundary" : "Browser preview boundary"}
        </div>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div>
            <span className="eyebrow">FICSIT TREATY ENFORCEMENT PLATFORM</span>
            <span className="document-code">FORM FTEP-01 / REV 2</span>
          </div>
          <div className="topbar-status">
            <span>ONBOARDING</span>
            <strong>{String(currentIndex + 1).padStart(2, "0")} / {String(stages.length).padStart(2, "0")}</strong>
          </div>
        </header>

        <div className="progress-track" aria-label={`${Math.round(stageProgress(state.stage))}% complete`}>
          <span style={{ width: `${stageProgress(state.stage)}%` }} />
        </div>

        <div className="content-frame">
          <article className="stage-card">
            <div className="stage-heading">
              <span className="section-code">SECTION {String(currentIndex + 1).padStart(2, "0")}</span>
              <h1>{currentStage.title}</h1>
              <p>{currentStage.explanation}</p>
            </div>

            <div className="stage-content">
              {state.stage === "welcome" && (
                <div className="welcome-grid">
                  <div className="hero-stamp" aria-hidden="true">
                    <span>FICSIT</span>
                    <strong>CLEARED</strong>
                    <small>FOR DIPLOMATIC ONBOARDING</small>
                  </div>
                  <div className="benefits-list">
                    <h2>The launcher handles:</h2>
                    <ul>
                      <li>Game-data validation and asset preparation</li>
                      <li>Device registration and treaty entitlement</li>
                      <li>Controller, graphics, and system checks</li>
                      <li>Updates, diagnostics, and eventual Zelda launch</li>
                    </ul>
                  </div>
                </div>
              )}

              {state.stage === "account-login" && (
                <div className="callout-panel">
                  <span className="panel-kicker">MILESTONE 1.5 ADAPTER</span>
                  <h2>{state.accountName ?? "Demonstration account available"}</h2>
                  <p>
                    Remote authentication is deliberately not mandatory yet. This local identity exercises the complete GUI path without pretending to be a production account.
                  </p>
                  {state.accountName && <StatusPill status="PASS" />}
                </div>
              )}

              {state.stage === "treaty-review" && (
                <>
                  <div className="treaty-summary">
                    <div>
                      <span>PROVIDER</span>
                      <strong>{treaty.provider}</strong>
                      {treaty.summary.provider.map((item) => <p key={item}>✓ {item}</p>)}
                    </div>
                    <div>
                      <span>BENEFICIARY</span>
                      <strong>{treaty.beneficiary}</strong>
                      {treaty.summary.beneficiary.map((item) => <p key={item}>✓ {item}</p>)}
                    </div>
                  </div>
                  <div className="treaty-scroll" tabIndex={0} aria-label="Full fourteen-article treaty">
                    <h2>{treaty.title}</h2>
                    <p className="treaty-disclaimer">{treaty.disclaimer}</p>
                    {treaty.articles.map((article) => (
                      <section key={article.number}>
                        <h3>ARTICLE {article.number} — {article.title}</h3>
                        {article.paragraphs.map((paragraph) => <p key={paragraph}>{paragraph}</p>)}
                      </section>
                    ))}
                  </div>
                </>
              )}

              {state.stage === "treaty-acceptance" && (
                <div className="acceptance-panel">
                  <div className="treaty-seal" aria-hidden="true">II</div>
                  <div>
                    <h2>Platform agreement acknowledgement</h2>
                    <p>{treaty.disclaimer}</p>
                    <label className="check-control">
                      <input
                        type="checkbox"
                        checked={acceptanceChecked}
                        onChange={(event) => setAcceptanceChecked(event.target.checked)}
                      />
                      <span>I have read enough of this nonsense to understand the deal.</span>
                    </label>
                  </div>
                </div>
              )}

              {state.stage === "game-data-selection" && (
                <div className="file-picker-panel">
                  <div className="file-glyph" aria-hidden="true">64</div>
                  <h2>{state.selectedFileLabel ?? "No game data selected"}</h2>
                  <p>Selection and validation happen locally. FTEP never uploads the file.</p>
                  <div className="inline-actions">
                    <button className="secondary-button" type="button" onClick={chooseGameData} disabled={busy}>
                      Select File
                    </button>
                    <button className="text-button" type="button" onClick={chooseSyntheticData} disabled={busy}>
                      Use synthetic demonstration data
                    </button>
                  </div>
                  {state.synthetic && <div className="demo-warning">NON-PLAYABLE DEMONSTRATION FIXTURE</div>}
                </div>
              )}

              {state.stage === "game-data-validation" && (
                state.validation ? (
                  <>
                    <ValidationResults report={state.validation} />
                    <div className={`result-banner ${state.validation.versionSupported ? "ready" : "blocked"}`}>
                      <strong>{state.validation.versionSupported ? "READY" : "GAME DATA NOT SUPPORTED"}</strong>
                      <span>{state.validation.detectedVersion ?? "FICSIT-0007"}</span>
                    </div>
                  </>
                ) : (
                  <div className="processing-placeholder">
                    <div className="scanner" aria-hidden="true" />
                    <h2>Ready to inspect {state.selectedFileLabel}</h2>
                    <p>The native validator reads the file once and compares it with Shipwright's supported catalog.</p>
                  </div>
                )
              )}

              {state.stage === "asset-import" && (
                <div className="callout-panel">
                  <span className="panel-kicker">ATOMIC IMPORT PIPELINE</span>
                  <h2>
                    {state.assetsImported
                      ? state.synthetic
                        ? "Synthetic assets prepared"
                        : `${state.assetArchiveName ?? "Game-data archive"} imported`
                      : "Original game data will remain untouched"}
                  </h2>
                  <p>
                    {state.synthetic
                      ? "The synthetic path creates no copyrighted assets and can never launch Zelda."
                      : "The maintained Shipwright extractor runs in an application-managed staging directory. Only verified completed output is promoted; failed or cancelled staging is discarded."}
                  </p>
                  {state.assetsImported && <StatusPill status="PASS" />}
                  {busy && !state.synthetic && (
                    <button className="secondary-button" type="button" onClick={() => void cancelImport()}>
                      Cancel Import
                    </button>
                  )}
                </div>
              )}

              {state.stage === "device-registration" && (
                <div className="identity-card">
                  <span>REGISTERED GAMING APPARATUS</span>
                  <strong>
                    {state.deviceRegistered
                      ? state.synthetic
                        ? "RYAN-DEMO-01"
                        : "RYAN-WINDOWS-LOCAL"
                      : "AWAITING REGISTRATION"}
                  </strong>
                  <p>No certificate or device identifier requires manual handling.</p>
                  {state.deviceRegistered && <StatusPill status="PASS" />}
                </div>
              )}

              {state.stage === "entitlement-activation" && (
                <div className="entitlement-meter">
                  <div className={`entitlement-orb ${state.entitlementActive ? "active" : "pending"}`} aria-hidden="true" />
                  <div>
                    <span>ZELDA RUNTIME AUTHORIZATION</span>
                    <strong>
                      {state.entitlementActive
                        ? state.synthetic
                          ? "DEMONSTRATION ACTIVE"
                          : "LOCAL MILESTONE ACTIVE"
                        : "PENDING"}
                    </strong>
                    <p>
                      {state.synthetic
                        ? "The synthetic entitlement cannot authorize a real runtime launch."
                        : "Milestone 1.5 uses a local launcher entitlement. A signed remote lease replaces it when the control plane becomes mandatory."}
                    </p>
                  </div>
                </div>
              )}

              {state.stage === "controller-check" && (
                <div className="hardware-panel">
                  <div className="controller-glyph" aria-hidden="true">+ · ·</div>
                  <div>
                    <h2>{state.synthetic ? "Controller workflow ready" : "Native controller probe queued"}</h2>
                    <p>
                      {state.synthetic
                        ? "The demonstration confirms the GUI path."
                        : "Pre-flight queries Windows XInput. No controller is a warning because keyboard controls remain available."}
                    </p>
                  </div>
                </div>
              )}

              {state.stage === "graphics-check" && (
                <div className="hardware-panel">
                  <div className="graphics-glyph" aria-hidden="true"><span /><span /><span /></div>
                  <div>
                    <h2>{state.synthetic ? "Graphics workflow ready" : "Native graphics probe queued"}</h2>
                    <p>
                      {state.synthetic
                        ? "The demonstration confirms the GUI path."
                        : "Pre-flight creates a hardware Direct3D 11 device and records the negotiated feature level."}
                    </p>
                  </div>
                </div>
              )}

              {state.stage === "system-diagnostics" && (
                state.diagnostics ? (
                  <>
                    <Diagnostics checks={state.diagnostics} />
                    <div className={`result-banner ${diagnosticsReady ? "ready" : "blocked"}`}>
                      <strong>
                        {diagnosticsReady
                          ? state.synthetic
                            ? "SYNTHETIC SYSTEM READY"
                            : "SYSTEM READY"
                          : "ACTION REQUIRED"}
                      </strong>
                      <span>
                        {diagnosticsReady
                          ? state.synthetic
                            ? "No launch-critical synthetic failures"
                            : "No launch-critical native failures"
                          : "Expand failed checks for details"}
                      </span>
                    </div>
                  </>
                ) : (
                  <div className="processing-placeholder">
                    <div className="radar" aria-hidden="true"><span /></div>
                    <h2>Pre-flight inspection standing by</h2>
                    <p>Checks run automatically through one launcher action and expose jargon only in expandable details.</p>
                  </div>
                )
              )}

              {state.stage === "ready" && (
                <div className="ready-panel">
                  <div className="ready-check" aria-hidden="true">✓</div>
                  <span>SYSTEM STATUS</span>
                  <h2>{state.synthetic ? "SYNTHETIC ONBOARDING COMPLETE" : "READY FOR ZELDA OPERATIONS"}</h2>
                  <p>
                    {state.synthetic
                      ? "The full GUI journey is operational. This demonstration does not contain, import, authorize, or launch Zelda."
                      : "Treaty, game data, entitlement, and runtime are ready."}
                  </p>
                </div>
              )}
            </div>

            {error && (
              <div className="error-panel" role="alert">
                <div>
                  <span>TECHNICAL IMPEDIMENT DETECTED</span>
                  <strong>{error}</strong>
                </div>
                <button type="button" onClick={copyDiagnostic}>Copy Diagnostic</button>
              </div>
            )}

            <details className="advanced-details">
              <summary>Advanced Details</summary>
              <dl>
                <div><dt>Stage</dt><dd>{state.stage}</dd></div>
                <div><dt>Boundary</dt><dd>{desktop ? "Tauri desktop" : "Browser preview"}</dd></div>
                <div><dt>Mode</dt><dd>{state.synthetic ? "Synthetic demonstration" : "Native user-data path"}</dd></div>
                {state.selectedFile && <div><dt>Selected path</dt><dd>{state.selectedFile}</dd></div>}
                {state.validation && <div><dt>SHA-1</dt><dd>{state.validation.sha1}</dd></div>}
                {state.treatyAcceptance && <div><dt>Treaty hash</dt><dd>{state.treatyAcceptance.sha256}</dd></div>}
              </dl>
            </details>

            <div className="commentary-strip">
              <span>FICSIT COMMENTARY</span>
              <p>{currentStage.commentary}</p>
            </div>

            <footer className="stage-actions">
              <button className="back-button" type="button" onClick={goBack} disabled={currentIndex === 0 || busy}>
                Back
              </button>
              <div className="autosave-note"><span /> Progress saved automatically</div>
              <button
                className="primary-button"
                type="button"
                onClick={() => void primary.action()}
                disabled={primary.disabled || busy}
              >
                {busy ? (state.stage === "asset-import" ? "Importing…" : "Working…") : primary.label}
              </button>
            </footer>
          </article>
        </div>
      </section>

      {showCompletion && (
        <div className="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="completion-title">
          <div className="completion-modal">
            <span className="modal-kicker">
              {state.synthetic ? "MILESTONE 1.5 VERTICAL SLICE" : "ZELDA RUNTIME DISPATCH"}
            </span>
            <h2 id="completion-title">
              {state.synthetic ? "Diplomatic onboarding demonstrated" : "Zelda is launching"}
            </h2>
            <p>
              {state.synthetic
                ? "The resumable launcher journey, native boundaries, treaty view, local validation contract, and pre-flight presentation are operational. Synthetic data can never launch Zelda."
                : `${launchReport?.runtimeId ?? "Runtime"} ${launchReport?.runtimeVersion ?? "unknown"} started as process ${launchReport?.processId ?? "unknown"} through runtime protocol ${launchReport?.runtimeProtocol ?? 1}.`}
            </p>
            <button
              className="primary-button"
              type="button"
              onClick={state.synthetic ? resetDemo : () => setShowCompletion(false)}
            >
              {state.synthetic ? "Run Again" : "Close"}
            </button>
          </div>
        </div>
      )}
    </main>
  );
}

export default App;
