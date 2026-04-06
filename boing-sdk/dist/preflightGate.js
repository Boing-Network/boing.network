/**
 * Fail fast before heavy flows (swap UI, indexer tick) when the RPC environment is unhealthy.
 */
import { doctorBoingRpcEnvironment } from './rpcDoctor.js';
export class BoingRpcPreflightError extends Error {
    constructor(doctor) {
        const msg = doctor.messages.length > 0
            ? doctor.messages.join('\n')
            : 'RPC environment check failed (see BoingRpcPreflightError.doctor).';
        super(msg);
        this.name = 'BoingRpcPreflightError';
        this.doctor = doctor;
        Object.setPrototypeOf(this, BoingRpcPreflightError.prototype);
    }
}
export function isBoingRpcPreflightError(e) {
    return e instanceof BoingRpcPreflightError;
}
/**
 * Runs {@link doctorBoingRpcEnvironment}; throws {@link BoingRpcPreflightError} when **`ok`** is false.
 * Use in dApp “connect” or before starting polling loops.
 */
export async function assertBoingRpcEnvironment(client, options) {
    const doctor = await doctorBoingRpcEnvironment(client, options);
    if (!doctor.ok) {
        throw new BoingRpcPreflightError(doctor);
    }
    return doctor;
}
