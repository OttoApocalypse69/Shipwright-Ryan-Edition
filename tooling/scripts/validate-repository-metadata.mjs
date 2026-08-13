import { readFile } from "node:fs/promises";

async function readJson(path) {
  return JSON.parse(await readFile(new URL(path, import.meta.url), "utf8"));
}

const catalog = await readJson("../../packages/game-catalog/catalog.v1.json");
const components = await readJson("../../packages/third-party/components.v1.json");
const runtimeManifests = await readJson("../../runtimes/manifests/runtime-manifests.v1.json");

for (const [name, document] of [
  ["game catalog", catalog],
  ["third-party registry", components],
  ["runtime manifests", runtimeManifests],
]) {
  if (document.schemaVersion !== 1) {
    throw new Error(`${name} must use schemaVersion 1`);
  }
}

function unique(values, label) {
  const seen = new Set();
  for (const value of values) {
    if (seen.has(value)) throw new Error(`${label} contains duplicate ${value}`);
    seen.add(value);
  }
  return seen;
}

const gameIds = unique(catalog.games.map((game) => game.id), "game catalog");
if (gameIds.size !== 4) throw new Error("initial game catalog must contain exactly four scoped targets");

const runtimeIds = unique(
  runtimeManifests.runtimes.map((runtime) => runtime.runtimeId),
  "runtime manifests",
);
for (const game of catalog.games) {
  for (const variant of game.variants) {
    for (const candidate of variant.runtimeCandidates) {
      if (!runtimeIds.has(candidate.runtimeId)) {
        throw new Error(`${game.id}/${variant.id} references missing runtime ${candidate.runtimeId}`);
      }
    }
  }
}

unique(components.components.map((component) => component.id), "third-party registry");
for (const component of components.components) {
  if (component.license === "UNKNOWN" && component.redistributionMode !== "BLOCKED_PENDING_REVIEW") {
    throw new Error(`${component.id} has an unknown license but is not blocked from redistribution`);
  }
}

for (const runtime of runtimeManifests.runtimes) {
  if (runtime.distribution.allowAutomaticDownload !== false) {
    throw new Error(`${runtime.runtimeId} unexpectedly permits automatic download`);
  }
}

console.log(
  `metadata ok: ${gameIds.size} games, ${runtimeIds.size} runtime manifests, ${components.components.length} third-party components`,
);
