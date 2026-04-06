/**
 * Access list helpers — pair with `boing_simulateTransaction` hints (Track A).
 */
import { validateHex32 } from './hex.js';
function normHex32(s) {
    return validateHex32(s);
}
/** Deduped sorted union of hex account ids (read ∪ write from suggestion). */
export function accountsFromSuggestedAccessList(s) {
    const set = new Set();
    for (const x of s.read)
        set.add(normHex32(x));
    for (const x of s.write)
        set.add(normHex32(x));
    return [...set].sort();
}
/**
 * Merge explicit read/write lists with simulation `suggested_access_list` (union per side).
 * Use after a successful or failed simulate to widen lists toward the heuristic minimum.
 */
export function mergeAccessListWithSimulation(read, write, sim) {
    const sug = sim.suggested_access_list;
    if (!sug) {
        return { read: [...read], write: [...write] };
    }
    const r = new Set(read.map(normHex32));
    const w = new Set(write.map(normHex32));
    for (const x of sug.read)
        r.add(normHex32(x));
    for (const x of sug.write)
        w.add(normHex32(x));
    return { read: [...r], write: [...w] };
}
/** Build `AccessList`-shaped object from simulation only (both sides = union of suggested read+write). */
export function accessListFromSimulation(sim) {
    const sug = sim.suggested_access_list;
    if (!sug)
        return null;
    const ids = accountsFromSuggestedAccessList(sug);
    return { read: ids, write: ids };
}
/** True if simulation reports the signed tx’s access list covers the heuristic minimum. */
export function simulationCoversSuggestedAccessList(sim) {
    return sim.access_list_covers_suggestion === true;
}
