/**
 * Single source of truth for faucet copy on the website.
 *
 * `FAUCET_AMOUNT_BOING` matches `FAUCET_DISPENSE_AMOUNT` in `crates/boing-node/src/faucet.rs`
 * (20 BOING per request, 1 request per 60 s per address). The confirmation shown after a request
 * always reports the amount the node actually sent, so a node that has not been redeployed yet
 * may still report a lower number.
 */
export const FAUCET_AMOUNT_BOING = 20;
export const FAUCET_COOLDOWN_SECONDS = 60;
