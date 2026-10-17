//! Native BOING denomination: 18 decimals (1 BOING = 10^18 base units).
//!
//! Ledger amounts (balances, stake, bonds, tx amounts, fees) are base units. Protocol constants and
//! node configuration that name a price (open-validator bond, name rent, faucet amount, QA rewards,
//! minimum stake) stay in **whole BOING** and are converted with [`Denomination::scale`] by the code
//! that charges them, so the same config works before and after a network's switch.
//!
//! The public testnet (chain 6913) ran in whole BOING until its migration height; every other chain
//! uses base units from genesis (see `boing_execution::ExecutionHistory::base_units_from`).

/// Decimals of native BOING.
pub const BOING_DECIMALS: u32 = 18;

/// Base units in one BOING (10^18).
pub const BASE_UNITS_PER_BOING: u128 = 1_000_000_000_000_000_000;

/// Unit of the native ledger at a given height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Denomination {
    /// Historical: amounts are whole BOING (testnet before its migration height).
    WholeBoing,
    /// Amounts are base units (10^-18 BOING).
    BaseUnits,
}

impl Denomination {
    /// Ledger units in one BOING.
    pub const fn unit(self) -> u128 {
        match self {
            Denomination::WholeBoing => 1,
            Denomination::BaseUnits => BASE_UNITS_PER_BOING,
        }
    }

    /// Convert a whole-BOING constant (bond, rent, faucet amount, ...) into ledger units.
    pub const fn scale(self, whole_boing: u128) -> u128 {
        whole_boing.saturating_mul(self.unit())
    }

    pub const fn decimals(self) -> u32 {
        match self {
            Denomination::WholeBoing => 0,
            Denomination::BaseUnits => BOING_DECIMALS,
        }
    }
}

/// Format base units as a decimal BOING string without trailing zeros (`1500000000000000000` -> `"1.5"`).
pub fn format_boing(base_units: u128) -> String {
    let whole = base_units / BASE_UNITS_PER_BOING;
    let frac = base_units % BASE_UNITS_PER_BOING;
    if frac == 0 {
        return whole.to_string();
    }
    let digits = format!("{frac:018}");
    format!("{whole}.{}", digits.trim_end_matches('0'))
}

/// Error from [`parse_boing`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseBoingError {
    #[error("empty amount")]
    Empty,
    #[error("invalid amount: {0}")]
    Invalid(String),
    #[error("more than 18 decimal places")]
    TooPrecise,
    #[error("amount too large")]
    Overflow,
}

/// Parse a decimal BOING amount (`"1.5"`, `"20"`, `"0.000001"`) into base units. No signs, exponents
/// or thousands separators.
pub fn parse_boing(s: &str) -> Result<u128, ParseBoingError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ParseBoingError::Empty);
    }
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, f),
        None => (s, ""),
    };
    if whole.is_empty() && frac.is_empty() {
        return Err(ParseBoingError::Invalid(s.into()));
    }
    let all_digits = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
    if !all_digits(whole) || !all_digits(frac) {
        return Err(ParseBoingError::Invalid(s.into()));
    }
    if frac.len() > BOING_DECIMALS as usize {
        return Err(ParseBoingError::TooPrecise);
    }
    let w: u128 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| ParseBoingError::Overflow)?
    };
    let mut f: u128 = if frac.is_empty() {
        0
    } else {
        frac.parse().map_err(|_| ParseBoingError::Overflow)?
    };
    for _ in frac.len()..BOING_DECIMALS as usize {
        f *= 10;
    }
    w.checked_mul(BASE_UNITS_PER_BOING)
        .and_then(|x| x.checked_add(f))
        .ok_or(ParseBoingError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_and_unit() {
        assert_eq!(Denomination::WholeBoing.scale(100), 100);
        assert_eq!(
            Denomination::BaseUnits.scale(100),
            100 * BASE_UNITS_PER_BOING
        );
        assert_eq!(
            Denomination::BaseUnits.scale(u128::MAX),
            u128::MAX,
            "saturates"
        );
    }

    #[test]
    fn format_and_parse_round_trip() {
        for (s, v) in [
            ("0", 0u128),
            ("1", BASE_UNITS_PER_BOING),
            ("1.5", 1_500_000_000_000_000_000),
            ("0.000000000000000001", 1),
            ("0.021", 21_000_000_000_000_000),
            ("20", 20 * BASE_UNITS_PER_BOING),
            ("10000000", 10_000_000 * BASE_UNITS_PER_BOING),
        ] {
            assert_eq!(parse_boing(s), Ok(v), "{s}");
            assert_eq!(format_boing(v), s);
        }
        assert_eq!(parse_boing(".5"), Ok(BASE_UNITS_PER_BOING / 2));
        assert_eq!(parse_boing("2."), Ok(2 * BASE_UNITS_PER_BOING));
        assert_eq!(
            parse_boing("1.0000000000000000001"),
            Err(ParseBoingError::TooPrecise)
        );
        assert!(matches!(
            parse_boing("-1"),
            Err(ParseBoingError::Invalid(_))
        ));
        assert!(matches!(
            parse_boing("1e18"),
            Err(ParseBoingError::Invalid(_))
        ));
        assert!(matches!(parse_boing("."), Err(ParseBoingError::Invalid(_))));
        assert_eq!(parse_boing(""), Err(ParseBoingError::Empty));
        assert_eq!(
            parse_boing("999999999999999999999"),
            Err(ParseBoingError::Overflow)
        );
    }
}
