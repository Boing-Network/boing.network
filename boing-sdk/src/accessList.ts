/**
 * Access list helpers — pair with `boing_simulateTransaction` hints (Track A).
 */

import type { AccessListJson, SimulateResult } from './types.js';
import { validateHex32 } from './hex.js';

function normHex32(s: string): string {
  return validateHex32(s);
}

/** Deduped sorted union of hex account ids (read ∪ write from suggestion). */
export function accountsFromSuggestedAccessList(s: AccessListJson): string[] {
  const set = new Set<string>();
  for (const x of s.read) set.add(normHex32(x));
  for (const x of s.write) set.add(normHex32(x));
  return [...set].sort();
}

/**
 * Merge explicit read/write lists with simulation `suggested_access_list` (union per side).
 * Use after a successful or failed simulate to widen lists toward the heuristic minimum.
 */
export function mergeAccessListWithSimulation(
  read: string[],
  write: string[],
  sim: SimulateResult
): { read: string[]; write: string[] } {
  const sug = sim.suggested_access_list;
  if (!sug) {
    return { read: [...read], write: [...write] };
  }
  const r = new Set(read.map(normHex32));
  const w = new Set(write.map(normHex32));
  for (const x of sug.read) r.add(normHex32(x));
  for (const x of sug.write) w.add(normHex32(x));
  return { read: [...r], write: [...w] };
}

/** Build `AccessList`-shaped object from simulation only (both sides = union of suggested read+write). */
export function accessListFromSimulation(sim: SimulateResult): { read: string[]; write: string[] } | null {
  const sug = sim.suggested_access_list;
  if (!sug) return null;
  const ids = accountsFromSuggestedAccessList(sug);
  return { read: ids, write: ids };
}

/** True if simulation reports the signed tx’s access list covers the heuristic minimum. */
export function simulationCoversSuggestedAccessList(sim: SimulateResult): boolean {
  return sim.access_list_covers_suggestion === true;
}
