/**
 * Single source of truth for faucet copy on the website.
 *
 * `FAUCET_AMOUNT_BOING` matches `FAUCET_DISPENSE_AMOUNT` in `crates/boing-node/src/faucet.rs`
 * (20 BOING per request, 1 request per 24 h per address; tune with BOING_FAUCET_COOLDOWN_SECS on the node
 * and FAUCET_ADDRESS_COOLDOWN_SECS / FAUCET_IP_MAX_PER_DAY on the public RPC edge). The confirmation shown after a request
 * always reports the amount the node actually sent, so a node that has not been redeployed yet
 * may still report a lower number.
 */
export const FAUCET_AMOUNT_BOING = 20;
export const FAUCET_COOLDOWN_SECONDS = 86_400;
/** Human label for the cooldown, used in page copy. */
export const FAUCET_COOLDOWN_LABEL = '24 hours';
/** Successful faucet requests allowed per network (IP) per day at the public RPC edge. */
export const FAUCET_IP_MAX_PER_DAY = 3;
