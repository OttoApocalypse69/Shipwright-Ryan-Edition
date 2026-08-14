import { useEffect, useMemo, useState } from "react";
import { advanceStage, initialOnboardingState, previousStage, restoreOnboardingState, stageProgress, stages, type OnboardingState } from "./domain/onboarding";
import { achievementCount, activeRunningGameSessions, cancelRegisteredGameLaunch, chooseGameSource, chooseRuntime, chooseShipwrightData, chooseSwitchData, chooseWiiUData, connectFtep, entitlementStatus, exportDiagnostics, ftepWebUrl, getCatalog, getDeviceIdentity, hideOverlay, importShipwrightData, isDesktopLauncher, launchRegisteredGame, loadLibrary, loadOnboarding, loadSessions, popOverlay, registerGame, registerImportedShipwrightGame, runDoctor, saveOnboarding, stopRunningGame, validateShipwrightData, type CatalogGame, type DoctorReport, type GameCatalog, type LibraryState, type OverlayNotification, type RuntimeSession, type ValidationReport } from "./lib/bridge";
import "./styles.css";

function OverlayApp() {
  const [notification, setNotification] = useState<OverlayNotification | null>(null);
  useEffect(() => {
    if (notification) {
      const timeout = setTimeout(() => setNotification(null), notification.displayMs);
      return () => clearTimeout(timeout);
    }

    let active = true;
    const poll = async () => {
      const next = await popOverlay().catch(() => null);
      if (!active) return;
      if (next) setNotification(next);
      else void hideOverlay();
    };
    const interval = setInterval(() => void poll(), 350);
    void poll();
    return () => { active = false; clearInterval(interval); };
  }, [notification]);
  return <main className="overlay-root">{notification && <article className="overlay-toast"><span>{notification.kind}</span><strong>{notification.title}</strong><p>{notification.message}</p></article>}</main>;
}

function FirstRun({ state, setState }: { state: OnboardingState; setState: (state: OnboardingState) => void }) {
  const [accepted, setAccepted] = useState(state.accordAccepted); const [busy, setBusy] = useState(false); const [connectionError, setConnectionError] = useState<string>(); const [doctor, setDoctor] = useState<DoctorReport>(); const [catalogCount, setCatalogCount] = useState(0); const stage = stages.find((value) => value.id === state.stage) ?? stages[0];
  useEffect(() => { if (state.stage === "library") void getCatalog().then((value) => setCatalogCount(value.games.length)); if (state.stage === "system") void runDoctor().then(setDoctor); }, [state.stage]);
  const next = async () => {
    if (state.stage === "account" && !state.accountMode) return;
    if (state.stage === "accord" && !accepted) return;
    if (state.stage === "device") { setBusy(true); try { const identity = await getDeviceIdentity(); const updated = { ...state, deviceId: identity.deviceId }; setState(advanceStage(updated)); } finally { setBusy(false); } return; }
    if (state.stage === "ready") { setState({ ...state, complete: true }); return; }
    setState(advanceStage({ ...state, accordAccepted: accepted }));
  };
  return <main className="first-run"><section className="onboarding-card"><div className="progress"><span style={{ width: `${stageProgress(state.stage)}%` }} /></div><div className="wordmark">SRE <small>SUPER RUNTIME ENVIRONMENT</small></div><span className="kicker">STEP {stages.indexOf(stage) + 1} OF {stages.length}</span><h1>{stage.title}</h1><p className="lede">{stage.explanation}</p>
    {state.stage === "welcome" && <div className="feature-grid"><div><b>Guided setup</b><span>No terminal required.</span></div><div><b>Local first</b><span>Your game data stays local.</span></div><div><b>Honest status</b><span>Experimental paths are labeled.</span></div></div>}
    {state.stage === "account" && <><div className="choice-grid"><button className="choice" disabled={busy} onClick={() => { setBusy(true); setConnectionError(undefined); void connectFtep().then(() => setState({ ...state, accountMode: "connected" })).catch((error) => setConnectionError(error instanceof Error ? error.message : String(error))).finally(() => setBusy(false)); }}><b>{busy ? "Waiting for browser…" : "Connect FTEP account"}</b><span>Browser sign-in, device registration, Accord ratification, and a signed callback. No pasted tokens.</span></button><button className="choice" onClick={() => setState({ ...state, accountMode: "offline" })}><b>Continue with offline setup</b><span>Library and diagnostics work; authorized launches need a signed cached lease.</span></button></div>{connectionError && <div className="error">{connectionError}</div>}</>}
    {state.stage === "accord" && <div className="accord"><b>The short version</b><p>Cooperate in good faith. Protect shared worlds and saves. Real life overrides the schedule. No mandatory marathon sessions. Suspension only blocks future FTEP-authorized launches.</p><label><input type="checkbox" checked={accepted} onChange={(event) => setAccepted(event.target.checked)} /> I accept version 3.0 of the Great Zelda–Satisfactory Accords.</label></div>}
    {state.stage === "device" && <div className="device-graphic"><span>LOCAL KEY</span><strong>Random device identity</strong><p>SRE generates the UUID and Ed25519 keypair when you continue.</p></div>}
    {state.stage === "library" && <div className="ready-box"><span>✓</span><div><b>{catalogCount || "…"} catalog titles loaded</b><p>No automatic drive scan. Configure any title later.</p></div></div>}
    {state.stage === "runtimes" && <div className="feature-grid"><div><b>Shipwright</b><span>Bundled native candidate</span></div><div><b>Wii U</b><span>Manual Cemu-compatible runtime</span></div><div><b>Switch</b><span>Manual generic external runtime</span></div></div>}
    {state.stage === "system" && <div className="doctor-list compact">{doctor?.checks.map((check) => <article key={check.id}><span className={`check ${check.status}`}>{check.status}</span><div><b>{check.id}</b><p>{check.summary}</p></div></article>) ?? <article><div><b>Running SRE Doctor…</b></div></article>}</div>}
    {state.stage === "ready" && <div className="ready-box"><span>✓</span><div><b>Setup complete</b><p>Pick a title from the data-driven library.</p></div></div>}
    <footer className="wizard-actions"><button className="button ghost" disabled={state.stage === "welcome" || busy} onClick={() => setState(previousStage(state))}>Back</button><button className="button" disabled={busy || (state.stage === "account" && !state.accountMode) || (state.stage === "accord" && !accepted)} onClick={() => void next()}>{state.stage === "ready" ? "Open library" : state.stage === "device" && busy ? "Creating identity…" : "Continue"}</button></footer>
  </section></main>;
}

function SetupModal({ game, close, completed }: { game: CatalogGame; close: () => void; completed: () => void }) {
  const variant = game.variants.find((value) => value.id === game.preferredVariant) ?? game.variants[0]; const runtime = variant.runtimeCandidates.find((value) => value.runtimeId === variant.preferredRuntime) ?? variant.runtimeCandidates[0];
  const [source, setSource] = useState<string>(); const [runtimePath, setRuntimePath] = useState<string>(); const [validation, setValidation] = useState<ValidationReport>(); const [busy, setBusy] = useState(false); const [error, setError] = useState<string>(); const shipwrightRuntime = runtime.runtimeId === "shipwright"; const nativeRomRuntime = shipwrightRuntime || runtime.runtimeId === "two-ship"; const managedRuntime = nativeRomRuntime || runtime.runtimeId === "cemu-compatible" || runtime.runtimeId === "switch-runtime";
  const selectSource = async () => { setError(undefined); const path = nativeRomRuntime ? await chooseShipwrightData(game.title) : runtime.runtimeId === "cemu-compatible" ? await chooseWiiUData() : runtime.runtimeId === "switch-runtime" ? await chooseSwitchData() : await chooseGameSource(false); if (path) { setSource(path); if (shipwrightRuntime) { setBusy(true); try { setValidation(await validateShipwrightData(path)); } catch (value) { setError(String(value)); } finally { setBusy(false); } } } };
  const finish = async () => { if (!source) return; setBusy(true); setError(undefined); try { if (shipwrightRuntime) { if (!validation?.integrityValidated || !validation.importPipelineAvailable) throw new Error("Selected game data is not supported by the native import pipeline."); await importShipwrightData(source, validation); await registerImportedShipwrightGame(); } else if (runtime.runtimeId === "two-ship" || runtime.runtimeId === "cemu-compatible" || runtime.runtimeId === "switch-runtime") { await registerGame({ gameId: game.id, variantId: variant.id, runtimeId: runtime.runtimeId, gameSource: source }); } else { if (!runtimePath) throw new Error("Choose your external runtime executable."); await registerGame({ gameId: game.id, variantId: variant.id, runtimeId: runtime.runtimeId, gameSource: source, runtimeExecutable: runtimePath, switchImplementationId: "ryujinx" }); } completed(); } catch (value) { setError(value instanceof Error ? value.message : String(value)); } finally { setBusy(false); } };
  return <div className="modal-backdrop"><section className="setup-modal" role="dialog" aria-modal="true"><button className="close" onClick={close}>×</button><span className="kicker">GAME SETUP</span><h2>{game.title}</h2><p>{game.description}</p><div className="setup-step"><span>1</span><div><b>Game data you provide</b><p>{variant.sourceRequirements[0]?.description}</p><button className="button ghost" onClick={() => void selectSource()}>{source ? "Change selection" : runtime.runtimeId === "cemu-compatible" ? "Choose Wii U game file" : "Choose game file"}</button>{source && <code>{source}</code>}{validation && <small className={validation.integrityValidated ? "ok" : "bad"}>{validation.integrityValidated ? `Validated: ${validation.detectedVersion}` : "Unsupported game-data hash"}</small>}</div></div>{(runtime.runtimeId === "two-ship" || runtime.runtimeId === "cemu-compatible") && <div className="setup-step"><span>2</span><div><b>Managed runtime</b><p>{runtime.runtimeId === "two-ship" ? "SRE includes the reviewed 2 Ship 2 Harkinian runtime. Your selected ROM stays where it is; the runtime creates its own generated game archive on first launch." : "SRE installed Cemu for this Windows account. Select your legally dumped Wii U .wua, .wux, or .wud game image, or an RPX title file; SRE does not install or retrieve game data, keys, or firmware."}</p></div></div>}{!managedRuntime && <div className="setup-step"><span>2</span><div><b>External runtime</b><p>SRE does not bundle or download this runtime. Select the executable you installed separately.</p><button className="button ghost" onClick={async () => { const path = await chooseRuntime(); if (path) setRuntimePath(path); }}>{runtimePath ? "Change runtime" : "Choose runtime executable"}</button>{runtimePath && <code>{runtimePath}</code>}</div></div>}<div className="status-line"><span className={`badge ${runtime.compatibility}`}>{runtime.compatibility}</span><span>{runtime.runtimeId} · {variant.originalPlatform.replaceAll("_", " ")}</span></div>{error && <div className="error">{error}</div>}<footer className="wizard-actions"><button className="button ghost" onClick={close}>Cancel</button><button className="button" disabled={busy || !source || (!managedRuntime && !runtimePath)} onClick={() => void finish()}>{busy ? "Validating…" : shipwrightRuntime ? "Import safely" : runtime.runtimeId === "two-ship" ? "Add Majora's Mask" : runtime.runtimeId === "cemu-compatible" ? "Add Breath of the Wild" : "Register game"}</button></footer></section></div>;
}

function LibraryApp() {
  const [catalog, setCatalog] = useState<GameCatalog>(); const [library, setLibrary] = useState<LibraryState>({ installations: {} }); const [setup, setSetup] = useState<CatalogGame>(); const [view, setView] = useState<"library" | "diagnostics">("library"); const [doctor, setDoctor] = useState<DoctorReport>(); const [launchError, setLaunchError] = useState<string>(); const [launchingGameId, setLaunchingGameId] = useState<string>(); const [connectionError, setConnectionError] = useState<string>(); const [connectionBusy, setConnectionBusy] = useState(false); const [sessions, setSessions] = useState<RuntimeSession[]>([]); const [unlocks, setUnlocks] = useState(0); const [entitlement, setEntitlement] = useState("OFFLINE / NOT AUTHORIZED"); const [ftepUrl, setFtepUrl] = useState("http://localhost:3000"); const [search, setSearch] = useState(""); const [filter, setFilter] = useState<"all" | "installed" | "setup">("all");
  const refresh = async () => { const [nextCatalog, nextLibrary, nextSessions, nextUnlocks] = await Promise.all([getCatalog(), loadLibrary(), loadSessions(), achievementCount()]); setCatalog(nextCatalog); setLibrary(nextLibrary); setSessions(nextSessions.sessions); setUnlocks(nextUnlocks); void entitlementStatus().then(() => setEntitlement("OFFLINE AUTHORIZED")).catch(() => setEntitlement("OFFLINE / NOT AUTHORIZED")); };
  const connect = async () => { setConnectionBusy(true); setConnectionError(undefined); try { await connectFtep(); await refresh(); } catch (value) { setConnectionError(value instanceof Error ? value.message : String(value)); } finally { setConnectionBusy(false); } };
  useEffect(() => { void refresh(); void ftepWebUrl().then(setFtepUrl).catch((error) => setConnectionError(error instanceof Error ? error.message : String(error))); }, []);
  const configured = useMemo(() => new Map(Object.values(library.installations).filter((value) => value.status === "READY").map((value) => [value.gameId, value])), [library]);
  const visibleGames = useMemo(() => catalog?.games.filter((game) => game.title.toLowerCase().includes(search.toLowerCase()) && (filter === "all" || (filter === "installed" ? configured.has(game.id) : !configured.has(game.id)))) ?? [], [catalog, configured, filter, search]);
  const sessionFor = (gameId: string) => sessions.filter((session) => session.gameId === gameId).sort((a, b) => b.requestedAtUnixMs - a.requestedAtUnixMs)[0];
  const totalPlaytime = sessions.reduce((sum, session) => sum + (session.durationMs ?? 0), 0);
  const formatPlaytime = (durationMs: number) => `${Math.floor(durationMs / 3_600_000)}h ${Math.floor(durationMs % 3_600_000 / 60_000)}m`;
  const play = async (gameId: string) => { const installation = configured.get(gameId); if (!installation || launchingGameId) return; setLaunchError(undefined); setLaunchingGameId(gameId); try { await launchRegisteredGame(installation.installationId); await refresh(); } catch (value) { setLaunchError(value instanceof Error ? value.message : String(value)); } finally { setLaunchingGameId(undefined); } };
  return <main className="app-shell"><aside className="sidebar"><div className="wordmark">SRE <small>SUPER RUNTIME ENVIRONMENT</small></div><nav><button className={view === "library" ? "active" : ""} onClick={() => setView("library")}>Library</button><button className={view === "diagnostics" ? "active" : ""} onClick={() => { setView("diagnostics"); void runDoctor().then(setDoctor); }}>Diagnostics</button><button disabled={connectionBusy} onClick={() => void connect()}>{connectionBusy ? "Waiting for FTEP..." : "Connect / refresh FTEP"}</button><a href={`${ftepUrl}/dashboard`} target="_blank" rel="noreferrer">FTEP Dashboard ↗</a></nav><div className="sidebar-foot"><span className="online-dot" /> Local services ready<small>{isDesktopLauncher() ? "Windows desktop" : "Browser preview"}</small></div></aside><section className="workspace">
    {view === "library" ? <><header className="workspace-header"><div><span className="kicker">YOUR GAMES</span><h1>Library</h1><p>One library. Multiple runtimes. Far too much infrastructure.</p></div><span className="catalog-count">{catalog?.games.length ?? 0} titles</span></header><div className="library-status"><span>FTEP <b>{entitlement}</b></span><span>Accord <b>RATIFIED LOCALLY</b></span><span>Achievements <b>{unlocks} / 10</b></span><span>Playtime <b>{Math.floor(totalPlaytime / 3_600_000)}h {Math.floor(totalPlaytime % 3_600_000 / 60_000)}m</b></span></div><div className="library-tools"><input aria-label="Search games" placeholder="Search games" value={search} onChange={(event) => setSearch(event.target.value)} /><button className={filter === "all" ? "active" : ""} onClick={() => setFilter("all")}>All</button><button className={filter === "installed" ? "active" : ""} onClick={() => setFilter("installed")}>Installed</button><button className={filter === "setup" ? "active" : ""} onClick={() => setFilter("setup")}>Setup required</button></div>{connectionError && <div className="error">{connectionError}</div>}{launchError && <div className="error">{launchError}</div>}{launchingGameId && <div className="status-line" role="status">Preparing game files in the background…</div>}<div className="game-grid">{visibleGames.map((game) => { const recent = sessionFor(game.id); const launching = launchingGameId === game.id; return <article className="game-card" key={game.id}><div className="game-cover" style={{ background: game.cover.backgroundColor, color: game.cover.accentColor }}>{game.cover.titleMark}<span>{game.variants[0].originalPlatform.replaceAll("_", " ")}</span></div><div className="game-body"><span className={`badge ${game.variants[0].runtimeCandidates[0].compatibility}`}>{game.variants[0].runtimeCandidates[0].compatibility}</span><h3>{game.title}</h3><p>{game.description}</p>{recent && <small className="last-played">Last played {new Date(recent.requestedAtUnixMs).toLocaleDateString()} · {Math.floor((recent.durationMs ?? 0) / 60_000)} min</small>}{configured.has(game.id) ? <div className="card-actions"><button className="button ghost" disabled={Boolean(launchingGameId)} onClick={() => setSetup(game)}>Setup</button><button className="button" disabled={Boolean(launchingGameId)} onClick={() => void play(game.id)}>{launching ? "Preparing game…" : "Play"}</button></div> : <button className="button" disabled={Boolean(launchingGameId)} onClick={() => setSetup(game)}>Set up</button>}</div></article>; })}</div></> : <><header className="workspace-header"><div><span className="kicker">SRE DOCTOR</span><h1>Diagnostics</h1><p>Actionable checks with sanitized export.</p></div><button className="button ghost" onClick={() => void exportDiagnostics()}>Export sanitized report</button></header><div className="doctor-list">{doctor?.checks.map((check) => <article key={check.id}><span className={`check ${check.status}`}>{check.status}</span><div><b>{check.id}</b><p>{check.summary}</p>{check.remediation && <small>{check.remediation}</small>}</div></article>)}</div></>}
  </section>{setup && <SetupModal game={setup} close={() => setSetup(undefined)} completed={() => { setSetup(undefined); void refresh(); }} />}</main>;
}

function LibraryAppV2() {
  const [catalog, setCatalog] = useState<GameCatalog>();
  const [library, setLibrary] = useState<LibraryState>({ installations: {} });
  const [setup, setSetup] = useState<CatalogGame>();
  const [view, setView] = useState<"library" | "diagnostics">("library");
  const [doctor, setDoctor] = useState<DoctorReport>();
  const [launchError, setLaunchError] = useState<string>();
  const [launchingGameId, setLaunchingGameId] = useState<string>();
  const [cancellingLaunch, setCancellingLaunch] = useState(false);
  const [stoppingSessionId, setStoppingSessionId] = useState<string>();
  const [connectionError, setConnectionError] = useState<string>();
  const [connectionBusy, setConnectionBusy] = useState(false);
  const [sessions, setSessions] = useState<RuntimeSession[]>([]);
  const [activeGameSessionIds, setActiveGameSessionIds] = useState<string[]>([]);
  const [unlocks, setUnlocks] = useState(0);
  const [entitlement, setEntitlement] = useState("OFFLINE / NOT AUTHORIZED");
  const [ftepUrl, setFtepUrl] = useState("http://localhost:3000");
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<"all" | "installed" | "setup">("all");

  const refresh = async () => {
    const [nextCatalog, nextLibrary, nextSessions, nextActiveGameSessionIds, nextUnlocks] = await Promise.all([
      getCatalog(), loadLibrary(), loadSessions(), activeRunningGameSessions(), achievementCount(),
    ]);
    setCatalog(nextCatalog);
    setLibrary(nextLibrary);
    setSessions(nextSessions.sessions);
    setActiveGameSessionIds(nextActiveGameSessionIds);
    setUnlocks(nextUnlocks);
    void entitlementStatus().then(() => setEntitlement("OFFLINE AUTHORIZED")).catch(() => setEntitlement("OFFLINE / NOT AUTHORIZED"));
  };
  const refreshLiveGameState = async () => {
    try {
      const [nextSessions, nextActiveGameSessionIds] = await Promise.all([
        loadSessions(), activeRunningGameSessions(),
      ]);
      setSessions(nextSessions.sessions);
      setActiveGameSessionIds(nextActiveGameSessionIds);
    } catch {
      // The next full refresh reports any actionable launcher error.
    }
  };

  const connect = async () => {
    setConnectionBusy(true);
    setConnectionError(undefined);
    try { await connectFtep(); await refresh(); }
    catch (value) { setConnectionError(value instanceof Error ? value.message : String(value)); }
    finally { setConnectionBusy(false); }
  };

  useEffect(() => {
    void refresh();
    void ftepWebUrl().then(setFtepUrl).catch((error) => setConnectionError(error instanceof Error ? error.message : String(error)));
  }, []);
  useEffect(() => {
    if (activeGameSessionIds.length === 0) return;
    const poll = window.setInterval(() => void refreshLiveGameState(), 500);
    return () => window.clearInterval(poll);
  }, [activeGameSessionIds.length]);

  const configured = useMemo(
    () => new Map(Object.values(library.installations).filter((value) => value.status === "READY").map((value) => [value.gameId, value])),
    [library],
  );
  const visibleGames = useMemo(
    () => catalog?.games.filter((game) => game.title.toLowerCase().includes(search.toLowerCase()) && (filter === "all" || (filter === "installed" ? configured.has(game.id) : !configured.has(game.id)))) ?? [],
    [catalog, configured, filter, search],
  );
  const sessionFor = (gameId: string) => sessions.filter((session) => session.gameId === gameId).sort((a, b) => b.requestedAtUnixMs - a.requestedAtUnixMs)[0];
  const totalPlaytime = sessions.reduce((sum, session) => sum + (session.durationMs ?? 0), 0);
  const formatPlaytime = (durationMs: number) => `${Math.floor(durationMs / 3_600_000)}h ${Math.floor(durationMs % 3_600_000 / 60_000)}m`;

  const play = async (gameId: string) => {
    const installation = configured.get(gameId);
    if (!installation || launchingGameId) return;
    setLaunchError(undefined);
    setLaunchingGameId(gameId);
    try { await launchRegisteredGame(installation.installationId); await refresh(); }
    catch (value) { setLaunchError(value instanceof Error ? value.message : String(value)); }
    finally { setLaunchingGameId(undefined); setCancellingLaunch(false); }
  };
  const cancelLaunch = async () => {
    setCancellingLaunch(true);
    setLaunchError(undefined);
    try { await cancelRegisteredGameLaunch(); }
    catch (value) { setLaunchError(value instanceof Error ? value.message : String(value)); setCancellingLaunch(false); }
  };
  const stopGame = async (session: RuntimeSession) => {
    setStoppingSessionId(session.sessionId);
    setLaunchError(undefined);
    try {
      await Promise.race([
        stopRunningGame(session.sessionId),
        new Promise<never>((_, reject) => window.setTimeout(() => reject(new Error("The stop request timed out. The launcher has refreshed its game state.")), 5_000)),
      ]);
      await new Promise((resolve) => setTimeout(resolve, 350));
      await refresh();
    } catch (value) {
      setLaunchError(value instanceof Error ? value.message : String(value));
    } finally {
      setStoppingSessionId(undefined);
    }
  };

  return <main className="app-shell">
    <aside className="sidebar">
      <div className="wordmark">SRE <small>SUPER RUNTIME ENVIRONMENT</small></div>
      <nav>
        <button className={view === "library" ? "active" : ""} onClick={() => setView("library")}>Library</button>
        <button className={view === "diagnostics" ? "active" : ""} onClick={() => { setView("diagnostics"); void runDoctor().then(setDoctor); }}>Diagnostics</button>
        <button disabled={connectionBusy} onClick={() => void connect()}>{connectionBusy ? "Waiting for FTEP..." : "Connect / refresh FTEP"}</button>
        <a href={`${ftepUrl}/dashboard`} target="_blank" rel="noreferrer">FTEP Dashboard ↗</a>
      </nav>
      <div className="sidebar-foot"><span className="online-dot" /> Local services ready<small>{isDesktopLauncher() ? "Windows desktop" : "Browser preview"}</small></div>
    </aside>
    <section className="workspace">
      {view === "library" ? <>
        <header className="workspace-header"><div><span className="kicker">YOUR GAMES</span><h1>Library</h1><p>One library. Multiple runtimes. Far too much infrastructure.</p></div><span className="catalog-count">{catalog?.games.length ?? 0} titles</span></header>
        <div className="library-status"><span>FTEP <b>{entitlement}</b></span><span>Accord <b>RATIFIED LOCALLY</b></span><span>Achievements <b>{unlocks} / 10</b></span><span>Playtime <b>{Math.floor(totalPlaytime / 3_600_000)}h {Math.floor(totalPlaytime % 3_600_000 / 60_000)}m</b></span></div>
        <div className="library-tools"><input aria-label="Search games" placeholder="Search games" value={search} onChange={(event) => setSearch(event.target.value)} /><button className={filter === "all" ? "active" : ""} onClick={() => setFilter("all")}>All</button><button className={filter === "installed" ? "active" : ""} onClick={() => setFilter("installed")}>Installed</button><button className={filter === "setup" ? "active" : ""} onClick={() => setFilter("setup")}>Setup required</button></div>
        {connectionError && <div className="error">{connectionError}</div>}
        {launchError && <div className="error">{launchError}</div>}
        {launchingGameId && <div className="launch-status" role="status"><span>{cancellingLaunch ? "Cancelling preparation..." : "Preparing game files in the background..."}</span><button className="button ghost" disabled={cancellingLaunch} onClick={() => void cancelLaunch()}>{cancellingLaunch ? "Cancelling..." : "Cancel launch"}</button></div>}
        <div className="game-grid">{visibleGames.map((game) => {
          const recent = sessionFor(game.id);
          const gamePlaytime = sessions.reduce((sum, session) => session.gameId === game.id ? sum + (session.durationMs ?? 0) : sum, 0);
          const launching = launchingGameId === game.id;
          const running = Boolean(recent?.processId && activeGameSessionIds.includes(recent.sessionId));
          const stopping = Boolean(stoppingSessionId && recent?.sessionId === stoppingSessionId);
          const coverStyle = {
            backgroundColor: game.cover.backgroundColor,
            color: game.cover.accentColor,
            ...(game.cover.bannerUrl ? { backgroundImage: `linear-gradient(90deg, rgba(7, 17, 17, .76), rgba(7, 17, 17, .16)), url("${game.cover.bannerUrl}")` } : {}),
          };
          return <article className="game-card" key={game.id}>
            <div className={`game-cover${game.cover.bannerUrl ? " has-banner" : ""}`} style={coverStyle}>{game.cover.titleMark}<span>{game.variants[0].originalPlatform.replaceAll("_", " ")}</span></div>
            <div className="game-body">
              <span className={`badge ${game.variants[0].runtimeCandidates[0].compatibility}`}>{game.variants[0].runtimeCandidates[0].compatibility}</span>
              <h3>{game.title}</h3><p>{game.description}</p>
              {recent && <small className="last-played">Last played {new Date(recent.requestedAtUnixMs).toLocaleDateString()} · {Math.floor((recent.durationMs ?? 0) / 60_000)} min</small>}
              {configured.has(game.id) && <small className="game-playtime">Total playtime {formatPlaytime(gamePlaytime)}</small>}
              {configured.has(game.id) ? <div className="card-actions"><button className="button ghost" disabled={Boolean(launchingGameId) || stopping} onClick={() => setSetup(game)}>Setup</button><button className="button" disabled={Boolean(launchingGameId) || stopping} onClick={() => running && recent ? void stopGame(recent) : void play(game.id)}>{stopping ? "Stopping..." : running ? "Stop game" : launching ? "Preparing game..." : "Play"}</button></div> : <button className="button" disabled={Boolean(launchingGameId)} onClick={() => setSetup(game)}>Set up</button>}
            </div>
          </article>;
        })}</div>
      </> : <>
        <header className="workspace-header"><div><span className="kicker">SRE DOCTOR</span><h1>Diagnostics</h1><p>Actionable checks with sanitized export.</p></div><button className="button ghost" onClick={() => void exportDiagnostics()}>Export sanitized report</button></header>
        <div className="doctor-list">{doctor?.checks.map((check) => <article key={check.id}><span className={`check ${check.status}`}>{check.status}</span><div><b>{check.id}</b><p>{check.summary}</p>{check.remediation && <small>{check.remediation}</small>}</div></article>)}</div>
      </>}
    </section>
    {setup && <SetupModal game={setup} close={() => setSetup(undefined)} completed={() => { setSetup(undefined); void refresh(); }} />}
  </main>;
}

export default function App() {
  const overlay = new URLSearchParams(window.location.search).get("overlay") === "1"; const [state, setStateValue] = useState(initialOnboardingState); const [loaded, setLoaded] = useState(false);
  useEffect(() => { if (overlay) return; void (async () => { const native = await loadOnboarding(); const local = restoreOnboardingState(localStorage.getItem("sre.onboarding.v1")); setStateValue(native ?? local); setLoaded(true); })(); }, [overlay]);
  const setState = (next: OnboardingState) => { setStateValue(next); localStorage.setItem("sre.onboarding.v1", JSON.stringify(next)); void saveOnboarding(next); };
  if (overlay) return <OverlayApp />; if (!loaded) return <main className="loading">SRE</main>; return state.complete ? <LibraryAppV2 /> : <FirstRun state={state} setState={setState} />;
}
