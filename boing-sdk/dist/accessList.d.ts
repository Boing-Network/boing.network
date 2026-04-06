/**
 * Access list helpers — pair with `boing_simulateTransaction` hints (Track A).
 */
import type { AccessListJson, SimulateResult } from './types.js';
/** Deduped sorted union of hex account ids (read ∪ write from suggestion). */
export declare function accountsFromSuggestedAccessList(s: AccessListJson): string[];
/**
 * Merge explicit read/write lists with simulation `suggested_access_list` (union per side).
 * Use after a successful or failed simulate to widen lists toward the heuristic minimum.
 */
export declare function mergeAccessListWithSimulation(read: string[], write: string[], sim: SimulateResult): {
    read: string[];
    write: string[];
};
/** Build `AccessList`-shaped object from simulation only (both sides = union of suggested read+write). */
export declare function accessListFromSimulation(sim: SimulateResult): {
    read: string[];
    write: string[];
} | null;
/** True if simulation reports the signed tx’s access list covers the heuristic minimum. */
export declare function simulationCoversSuggestedAccessList(sim: SimulateResult): boolean;
//# sourceMappingURL=accessList.d.ts.map