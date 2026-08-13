import catalogDocument from "../../../packages/game-catalog/catalog.v1.json";

export type CompatibilityState = "UNSUPPORTED" | "INVESTIGATING" | "EXPERIMENTAL" | "SUPPORTED" | "DEGRADED" | "BROKEN" | "DEPRECATED";
export interface CatalogGame {
  id: string;
  franchise: string;
  title: string;
  description: string;
  cover: { kind: "ORIGINAL_PLACEHOLDER"; titleMark: string; accentColor: string; backgroundColor: string };
  variants: Array<{
    id: string;
    originalPlatform: string;
    runtimeCandidates: Array<{ runtimeId: string; priority: number; compatibility: CompatibilityState }>;
    preferredRuntime?: string | null;
    sourceRequirements: Array<{ id: string; kind: string; description: string; required: boolean }>;
  }>;
  preferredVariant: string;
  achievementNamespace: string;
}

export const catalog = catalogDocument as { schemaVersion: number; games: CatalogGame[] };
export function gameById(id: string): CatalogGame | undefined { return catalog.games.find((game) => game.id === id); }

export function compatibilityRows() {
  return catalog.games.flatMap((game) => game.variants.flatMap((variant) => variant.runtimeCandidates.map((runtime) => ({
    gameId: game.id,
    title: game.title,
    variant: variant.id,
    platform: variant.originalPlatform,
    runtimeId: runtime.runtimeId,
    status: runtime.compatibility,
  }))));
}
