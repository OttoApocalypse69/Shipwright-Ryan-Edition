import { createHash } from "node:crypto";
import accordDocument from "../../../packages/treaty/FICSIT-ACCORD-0001.v3.json";

function canonicalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entry]) => [key, canonicalize(entry)]),
    );
  }
  return value;
}

export const accord = accordDocument;
export const accordCanonicalJson = JSON.stringify(canonicalize(accordDocument));
export const accordDocumentHash = createHash("sha256").update(accordCanonicalJson).digest("hex");
