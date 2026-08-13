import type { FtepRole } from "@/lib/auth-config";

export type AdminAction = "view" | "compatibility.write" | "entitlement.suspend" | "entitlement.restore" | "release.publish" | "audit.view";

export function mayPerform(role: FtepRole, action: AdminAction): boolean {
  return role === "admin" && ["view", "compatibility.write", "entitlement.suspend", "entitlement.restore", "release.publish", "audit.view"].includes(action);
}

export function requireAdmin(role: FtepRole): void {
  if (!mayPerform(role, "view")) throw new Error("FTEP_ADMIN_REQUIRED");
}
