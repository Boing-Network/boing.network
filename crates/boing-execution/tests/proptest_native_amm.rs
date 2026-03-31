//! Property tests for native CP pool math (`constant_product_amount_out`).

use boing_execution::constant_product_amount_out;
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_amount_out_never_exceeds_reserve_out(
        ra in 1u64..50_000u64,
        rb in 1u64..50_000u64,
        dx in 1u64..50_000u64
    ) {
        let out = constant_product_amount_out(ra, rb, dx);
        prop_assert!(out <= rb);
    }

    #[test]
    fn prop_product_invariant_non_decreasing(
        ra in 100u64..10_000u64,
        rb in 100u64..10_000u64,
        dx in 1u64..5_000u64
    ) {
        let k0 = (ra as u128) * (rb as u128);
        let out = constant_product_amount_out(ra, rb, dx);
        let ra1 = ra.saturating_add(dx);
        let rb1 = rb.saturating_sub(out);
        let k1 = (ra1 as u128) * (rb1 as u128);
        prop_assert!(k1 >= k0, "rounded CP should not decrease invariant below k");
    }
}
