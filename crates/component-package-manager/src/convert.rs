//! Checked integer conversions for size and index metadata.
//!
//! These helpers replace silent saturating conversions (e.g.
//! `i64::try_from(len).unwrap_or(i64::MAX)`) that would corrupt size or layer
//! index metadata on overflow. Each helper surfaces a descriptive error via
//! `anyhow` instead.

use anyhow::{Context as _, Result};

/// Convert a layer position/index (`usize`) into the `i32` the database schema
/// uses, returning an error instead of silently saturating on overflow.
pub(crate) fn index_to_i32(index: usize) -> Result<i32> {
    i32::try_from(index).with_context(|| format!("layer index {index} exceeds i32::MAX"))
}

/// Convert a byte length (`usize`) into the `i64` the database schema uses for
/// layer/manifest sizes, returning an error instead of silently saturating.
pub(crate) fn len_to_i64(len: usize) -> Result<i64> {
    i64::try_from(len).with_context(|| format!("byte length {len} exceeds i64::MAX"))
}

/// Convert an aggregate size (`u64`) into the `i64` the database schema uses,
/// returning an error instead of silently saturating on overflow.
pub(crate) fn size_to_i64(size: u64) -> Result<i64> {
    i64::try_from(size).with_context(|| format!("size {size} exceeds i64::MAX"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_to_i32_in_range() {
        assert_eq!(index_to_i32(0).unwrap(), 0);
        assert_eq!(index_to_i32(42).unwrap(), 42);
        assert_eq!(
            index_to_i32(i32::MAX as usize).unwrap(),
            i32::MAX,
            "the largest valid index must round-trip"
        );
    }

    #[test]
    fn index_to_i32_overflow_errors() {
        let too_big = i32::MAX as usize + 1;
        let err = index_to_i32(too_big).expect_err("overflow must be rejected");
        assert!(
            err.to_string().contains("exceeds i32::MAX"),
            "error should carry overflow context, got: {err}"
        );
    }

    #[test]
    fn len_to_i64_in_range() {
        assert_eq!(len_to_i64(0).unwrap(), 0);
        assert_eq!(len_to_i64(1024).unwrap(), 1024);
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn len_to_i64_overflow_errors() {
        // `usize` only exceeds `i64::MAX` on 64-bit targets.
        let too_big = i64::MAX as usize + 1;
        let err = len_to_i64(too_big).expect_err("overflow must be rejected");
        assert!(
            err.to_string().contains("exceeds i64::MAX"),
            "error should carry overflow context, got: {err}"
        );
    }

    #[test]
    fn size_to_i64_in_range() {
        assert_eq!(size_to_i64(0).unwrap(), 0);
        assert_eq!(size_to_i64(i64::MAX as u64).unwrap(), i64::MAX);
    }

    #[test]
    fn size_to_i64_overflow_errors() {
        let too_big = i64::MAX as u64 + 1;
        let err = size_to_i64(too_big).expect_err("overflow must be rejected");
        assert!(
            err.to_string().contains("exceeds i64::MAX"),
            "error should carry overflow context, got: {err}"
        );
    }
}
