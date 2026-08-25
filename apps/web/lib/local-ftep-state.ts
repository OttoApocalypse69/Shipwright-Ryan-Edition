import { createHash, randomBytes } from "node:crypto";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { mkdir, readFile, rename, unlink, writeFile } from "node:fs/promises";
import { localAccountByEmail } from "@/lib/local-credentials";
import { signLease, type LeasePayload } from "@/lib/signing";

const stateVersion = 1;
const maxStateBytes = 4 * 1024 * 1024;
const defaultLocalLeaseTtlSecs = 30 * 24 * 60 * 60;

export type LocalSignedLease = ReturnType<typeof signLease>;

type LocalDevice = {
  deviceId: string;
  email: string;
  subjectId: string;
  publicKey: string;
  label: string | null;
  registeredAtUnixMs: number;
  revokedAtUnixMs?: number;
};

type LocalRatification = {
  email: string;
  deviceId: string;
  version: string;
  documentHash: string;
  acceptedAtUnixMs: number;
};

type LocalEntitlement = {
  email: string;
  key: string;
  state: "ACTIVE" | "SUSPENDED" | "EXPIRED";
  grantedAtUnixMs: number;
};

type LocalLeaseRecord = {
  email: string;
  deviceId: string;
  issuedAtUnixSecs: number;
  expiresAtUnixSecs: number;
  signedLease: LocalSignedLease;
};

type LocalAuditEvent = {
  action: string;
  email: string;
  targetId: string;
  createdAtUnixMs: number;
};

export type LocalFtepState = {
  version: typeof stateVersion;
  devices: LocalDevice[];
  ratifications: LocalRatification[];
  entitlements: LocalEntitlement[];
  leases: LocalLeaseRecord[];
  auditEvents: LocalAuditEvent[];
};

const emptyState = (): LocalFtepState => ({ version: stateVersion, devices: [], ratifications: [], entitlements: [], leases: [], auditEvents: [] });

function localStatePath(): string {
  return process.env.FTEP_LOCAL_CONTROL_PLANE_STORE
    ?? join(process.env.LOCALAPPDATA ?? homedir(), "Shipwright Ryan Edition", "ftep", "local-control-plane.json");
}

function normalizedEmail(value: string): string {
  return value.trim().toLowerCase();
}

function fallbackSubjectId(email: string): string {
  return createHash("sha256").update(`ftep-local:${email}`).digest("hex");
}

function isState(value: unknown): value is LocalFtepState {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<LocalFtepState>;
  return candidate.version === stateVersion
    && Array.isArray(candidate.devices)
    && Array.isArray(candidate.ratifications)
    && Array.isArray(candidate.entitlements)
    && Array.isArray(candidate.leases)
    && Array.isArray(candidate.auditEvents);
}

async function readState(): Promise<LocalFtepState> {
  try {
    const bytes = await readFile(/* turbopackIgnore: true */ localStatePath());
    if (bytes.length > maxStateBytes) throw new Error("The local FTEP state file is too large.");
    const parsed: unknown = JSON.parse(bytes.toString("utf8"));
    if (!isState(parsed)) throw new Error("The local FTEP state file is invalid.");
    return parsed;
  } catch (error: unknown) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return emptyState();
    throw error;
  }
}

async function writeState(state: LocalFtepState): Promise<void> {
  const destination = localStatePath();
  const temporary = `${destination}.${randomBytes(8).toString("hex")}.tmp`;
  await mkdir(dirname(destination), { recursive: true });
  try {
    await writeFile(temporary, `${JSON.stringify(state, null, 2)}\n`, { encoding: "utf8", mode: 0o600 });
    await rename(temporary, destination);
  } finally {
    await unlink(temporary).catch(() => undefined);
  }
}

let stateOperation = Promise.resolve();

async function withStateLock<T>(operation: (state: LocalFtepState) => Promise<T> | T): Promise<T> {
  const previous = stateOperation;
  let release!: () => void;
  stateOperation = new Promise<void>((resolve) => { release = resolve; });
  await previous;
  try {
    const state = await readState();
    const result = await operation(state);
    await writeState(state);
    return result;
  } finally {
    release();
  }
}

function recordAudit(state: LocalFtepState, action: string, email: string, targetId: string, now: number): void {
  state.auditEvents.push({ action, email, targetId, createdAtUnixMs: now });
  if (state.auditEvents.length > 200) state.auditEvents.splice(0, state.auditEvents.length - 200);
}

async function localIdentity(email: string): Promise<{ email: string; subjectId: string }> {
  const normalized = normalizedEmail(email);
  const account = await localAccountByEmail(normalized);
  return { email: normalized, subjectId: account?.id ?? fallbackSubjectId(normalized) };
}

export async function registerLocalDevice(input: { email: string; deviceId: string; publicKey: string; label?: string }): Promise<{ deviceId: string; registered: true }> {
  const identity = await localIdentity(input.email);
  const now = Date.now();
  return await withStateLock((state) => {
    const existing = state.devices.find((device) => device.deviceId === input.deviceId);
    if (existing) {
      existing.email = identity.email;
      existing.subjectId = identity.subjectId;
      existing.publicKey = input.publicKey;
      existing.label = input.label ?? null;
      existing.registeredAtUnixMs = now;
      delete existing.revokedAtUnixMs;
    } else {
      state.devices.push({ deviceId: input.deviceId, email: identity.email, subjectId: identity.subjectId, publicKey: input.publicKey, label: input.label ?? null, registeredAtUnixMs: now });
    }
    recordAudit(state, "device.register", identity.email, input.deviceId, now);
    return { deviceId: input.deviceId, registered: true as const };
  });
}

export async function revokeLocalDevice(input: { email: string; deviceId: string }): Promise<{ deviceId: string; revoked: true }> {
  const identity = await localIdentity(input.email);
  const now = Date.now();
  return await withStateLock((state) => {
    const device = state.devices.find((candidate) => candidate.deviceId === input.deviceId && candidate.email === identity.email && !candidate.revokedAtUnixMs);
    if (!device) throw new Error("FTEP_DEVICE_NOT_FOUND");
    device.revokedAtUnixMs = now;
    recordAudit(state, "device.revoke", identity.email, input.deviceId, now);
    return { deviceId: input.deviceId, revoked: true as const };
  });
}

export async function ratifyLocalDevice(input: { email: string; deviceId: string; version: string; documentHash: string }): Promise<{ treatyId: string; version: string; documentHash: string; ratified: true }> {
  const identity = await localIdentity(input.email);
  const now = Date.now();
  return await withStateLock((state) => {
    const device = state.devices.find((candidate) => candidate.deviceId === input.deviceId && candidate.email === identity.email && !candidate.revokedAtUnixMs);
    if (!device) throw new Error("Device is not registered to this local account.");
    const existing = state.ratifications.find((candidate) => candidate.deviceId === input.deviceId && candidate.email === identity.email);
    if (existing) {
      existing.version = input.version;
      existing.documentHash = input.documentHash;
      existing.acceptedAtUnixMs = now;
    } else {
      state.ratifications.push({ email: identity.email, deviceId: input.deviceId, version: input.version, documentHash: input.documentHash, acceptedAtUnixMs: now });
    }
    const entitlement = state.entitlements.find((candidate) => candidate.email === identity.email && candidate.key === "ftep.library.nintendo");
    if (entitlement) {
      entitlement.state = "ACTIVE";
      entitlement.grantedAtUnixMs = now;
    } else {
      state.entitlements.push({ email: identity.email, key: "ftep.library.nintendo", state: "ACTIVE", grantedAtUnixMs: now });
    }
    recordAudit(state, "treaty.ratify", identity.email, input.deviceId, now);
    return { treatyId: "FICSIT-ACCORD-0001", version: input.version, documentHash: input.documentHash, ratified: true as const };
  });
}

export async function issueLocalLease(input: { email: string; deviceId: string }): Promise<LocalSignedLease> {
  const identity = await localIdentity(input.email);
  return await withStateLock((state) => {
    const device = state.devices.find((candidate) => candidate.deviceId === input.deviceId && candidate.email === identity.email && !candidate.revokedAtUnixMs);
    const entitlement = state.entitlements.find((candidate) => candidate.email === identity.email && candidate.key === "ftep.library.nintendo" && candidate.state === "ACTIVE");
    if (!device || !entitlement) throw new Error("No active entitlement is available for this local device.");
    const issuedAtUnixSecs = Math.floor(Date.now() / 1000);
    const ttl = Number.parseInt(process.env.FTEP_LOCAL_LEASE_TTL_SECS ?? String(defaultLocalLeaseTtlSecs), 10);
    const expiresAtUnixSecs = issuedAtUnixSecs + (Number.isFinite(ttl) && ttl > 0 ? ttl : defaultLocalLeaseTtlSecs);
    const payload: LeasePayload = { leaseId: crypto.randomUUID(), subjectId: device.subjectId, deviceId: input.deviceId, entitlements: [entitlement.key], issuedAtUnixSecs, notBeforeUnixSecs: issuedAtUnixSecs - 30, expiresAtUnixSecs };
    const signedLease = signLease(payload);
    state.leases.push({ email: identity.email, deviceId: input.deviceId, issuedAtUnixSecs, expiresAtUnixSecs, signedLease });
    if (state.leases.length > 50) state.leases.splice(0, state.leases.length - 50);
    recordAudit(state, "entitlement.lease.issue", identity.email, input.deviceId, issuedAtUnixSecs * 1_000);
    return signedLease;
  });
}

export async function readLocalFtepState(): Promise<LocalFtepState> {
  return await readState();
}
