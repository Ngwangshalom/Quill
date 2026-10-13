//! Exact decimal numbers.
//!
//! Quill numbers are decimals, not binary floating point, so `0.1 + 0.2` is
//! exactly `0.3`. A number is stored as a whole number of "units" plus how many
//! digits sit after the decimal point: `12.50` is 1250 with scale 2.
//!
//! Adding, subtracting and multiplying are always exact. Dividing keeps up to
//! `DIVISION_DIGITS` digits after the point, rounding the last one half to even.

use std::cmp::Ordering;
use std::fmt;

/// How many digits after the decimal point a division keeps.
pub const DIVISION_DIGITS: u32 = 20;

#[derive(Debug, Clone, Copy)]
pub struct Decimal {
    units: i128,
    scale: u32,
}

/// Why an arithmetic step couldn't produce a number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MathError {
    /// The result has more digits than a Quill number can hold.
    TooLarge,
    DivideByZero,
}

impl Decimal {
    pub const ZERO: Decimal = Decimal { units: 0, scale: 0 };

    /// Reads a number written like `42` or `0.10`. `None` if it has too many digits.
    pub fn parse(written: &str) -> Option<Decimal> {
        let (whole, fraction) = written.split_once('.').unwrap_or((written, ""));
        if whole.is_empty() || !(whole.chars().chain(fraction.chars())).all(|c| c.is_ascii_digit())
        {
            return None;
        }
        let digits = format!("{whole}{fraction}");
        let units = digits.parse::<i128>().ok()?;
        Some(Decimal {
            units,
            scale: u32::try_from(fraction.len()).ok()?,
        })
    }

    pub fn from_int(value: i128) -> Decimal {
        Decimal {
            units: value,
            scale: 0,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.units == 0
    }

    pub fn add(self, other: Decimal) -> Result<Decimal, MathError> {
        let (a, b, scale) = align(self, other)?;
        let units = a.checked_add(b).ok_or(MathError::TooLarge)?;
        Ok(Decimal { units, scale })
    }

    pub fn subtract(self, other: Decimal) -> Result<Decimal, MathError> {
        let (a, b, scale) = align(self, other)?;
        let units = a.checked_sub(b).ok_or(MathError::TooLarge)?;
        Ok(Decimal { units, scale })
    }

    pub fn multiply(self, other: Decimal) -> Result<Decimal, MathError> {
        let (a, b) = (self.trimmed(), other.trimmed());
        let units = a.units.checked_mul(b.units).ok_or(MathError::TooLarge)?;
        let scale = a.scale + b.scale;
        // Keep the precision people wrote: 0.10 * 3 is 0.30, not 0.3.
        let wanted = self.scale.max(other.scale);
        Decimal { units, scale }.with_scale_at_least(wanted)
    }

    pub fn divide(self, other: Decimal) -> Result<Decimal, MathError> {
        if other.is_zero() {
            return Err(MathError::DivideByZero);
        }
        let (a, b) = (self.trimmed(), other.trimmed());
        // a / b = (a.units / 10^a.scale) / (b.units / 10^b.scale)
        //       = a.units * 10^(b.scale + digits) / (b.units * 10^a.scale) / 10^digits
        // Use as many result digits as fit, up to DIVISION_DIGITS.
        for digits in (0..=DIVISION_DIGITS).rev() {
            let Some(numerator) = pow10(b.scale + digits).and_then(|p| a.units.checked_mul(p))
            else {
                continue;
            };
            let Some(denominator) = pow10(a.scale).and_then(|p| b.units.checked_mul(p)) else {
                return Err(MathError::TooLarge);
            };
            let units = divide_rounding(numerator, denominator);
            let result = Decimal {
                units,
                scale: digits,
            }
            .trimmed();
            return result.with_scale_at_least(self.scale.max(other.scale).min(digits));
        }
        Err(MathError::TooLarge)
    }

    /// What is left over after dividing, with the sign of `self` (like `7 % 3` is 1).
    pub fn remainder(self, other: Decimal) -> Result<Decimal, MathError> {
        if other.is_zero() {
            return Err(MathError::DivideByZero);
        }
        let (a, b, scale) = align(self, other)?;
        Ok(Decimal {
            units: a % b,
            scale,
        })
    }

    pub fn negate(self) -> Result<Decimal, MathError> {
        let units = self.units.checked_neg().ok_or(MathError::TooLarge)?;
        Ok(Decimal {
            units,
            scale: self.scale,
        })
    }

    /// The same number without trailing zeros after the point: 1.500 becomes 1.5.
    fn trimmed(self) -> Decimal {
        let mut result = self;
        while result.scale > 0 && result.units % 10 == 0 {
            result.units /= 10;
            result.scale -= 1;
        }
        result
    }

    /// Adds trailing zeros until there are at least `scale` digits after the point.
    /// If that doesn't fit, the number is kept as it is (it is still exact).
    fn with_scale_at_least(self, scale: u32) -> Result<Decimal, MathError> {
        if self.scale >= scale {
            return Ok(self);
        }
        match pow10(scale - self.scale).and_then(|p| self.units.checked_mul(p)) {
            Some(units) => Ok(Decimal { units, scale }),
            None => Ok(self),
        }
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Decimal) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Decimal {}

impl PartialOrd for Decimal {
    fn partial_cmp(&self, other: &Decimal) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Decimal {
    fn cmp(&self, other: &Decimal) -> Ordering {
        let (a, b) = (self.trimmed(), other.trimmed());
        match align(a, b) {
            Ok((a, b, _)) => a.cmp(&b),
            // Only possible for numbers near the size limit: compare signs, then
            // whole parts, which can't overflow.
            Err(_) => {
                let whole_a = a.units / pow10(a.scale).unwrap_or(1);
                let whole_b = b.units / pow10(b.scale).unwrap_or(1);
                a.units
                    .signum()
                    .cmp(&b.units.signum())
                    .then(whole_a.cmp(&whole_b))
            }
        }
    }
}

/// Shows the number with the digits it has: 0.30, 42, -1.5.
impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let digits = self.units.unsigned_abs().to_string();
        let sign = if self.units < 0 { "-" } else { "" };
        let scale = self.scale as usize;
        if scale == 0 {
            return write!(f, "{sign}{digits}");
        }
        let padded = format!("{digits:0>width$}", width = scale + 1);
        let (whole, fraction) = padded.split_at(padded.len() - scale);
        write!(f, "{sign}{whole}.{fraction}")
    }
}

/// Both numbers as units with the same scale.
fn align(a: Decimal, b: Decimal) -> Result<(i128, i128, u32), MathError> {
    let scale = a.scale.max(b.scale);
    let widen = |d: Decimal| {
        pow10(scale - d.scale)
            .and_then(|p| d.units.checked_mul(p))
            .ok_or(MathError::TooLarge)
    };
    Ok((widen(a)?, widen(b)?, scale))
}

fn pow10(exponent: u32) -> Option<i128> {
    10i128.checked_pow(exponent)
}

/// `numerator / denominator`, rounding half to even.
fn divide_rounding(numerator: i128, denominator: i128) -> i128 {
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder == 0 {
        return quotient;
    }
    // Compare |2 * remainder| with |denominator| without overflowing.
    let remainder = remainder.unsigned_abs();
    let half = denominator.unsigned_abs();
    let twice = remainder.saturating_mul(2);
    let away = match twice.cmp(&half) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => quotient % 2 != 0,
    };
    if !away {
        return quotient;
    }
    if (numerator < 0) != (denominator < 0) {
        quotient - 1
    } else {
        quotient + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(written: &str) -> Decimal {
        if let Some(positive) = written.strip_prefix('-') {
            return Decimal::parse(positive).unwrap().negate().unwrap();
        }
        Decimal::parse(written).unwrap()
    }

    fn show(result: Result<Decimal, MathError>) -> String {
        result.unwrap().to_string()
    }

    #[test]
    fn parses_and_shows_numbers_as_written() {
        for written in ["0", "42", "0.10", "3.14159", "1000000", "0.001"] {
            assert_eq!(d(written).to_string(), written);
        }
        assert_eq!(d("-0.05").to_string(), "-0.05");
    }

    #[test]
    fn rejects_numbers_that_are_too_long() {
        assert!(Decimal::parse("1".repeat(38).as_str()).is_some());
        assert!(Decimal::parse("9".repeat(40).as_str()).is_none());
    }

    #[test]
    fn point_one_plus_point_two_is_point_three() {
        assert_eq!(show(d("0.1").add(d("0.2"))), "0.3");
        assert_eq!(d("0.1").add(d("0.2")).unwrap(), d("0.3"));
    }

    #[test]
    fn adding_keeps_the_most_digits() {
        assert_eq!(show(d("0.10").add(d("0.2"))), "0.30");
        assert_eq!(show(d("100").add(d("50"))), "150");
        assert_eq!(show(d("5").subtract(d("7.5"))), "-2.5");
    }

    #[test]
    fn multiplying_is_exact() {
        assert_eq!(show(d("0.10").multiply(d("3"))), "0.30");
        assert_eq!(show(d("1.5").multiply(d("1.5"))), "2.25");
        assert_eq!(show(d("100").multiply(d("1.05"))), "105.00");
        assert_eq!(show(d("-2").multiply(d("3"))), "-6");
    }

    #[test]
    fn dividing() {
        assert_eq!(show(d("10").divide(d("4"))), "2.5");
        assert_eq!(show(d("10").divide(d("2"))), "5");
        assert_eq!(show(d("1.00").divide(d("4"))), "0.25");
        assert_eq!(show(d("1").divide(d("3"))), "0.33333333333333333333");
        assert_eq!(show(d("2").divide(d("3"))), "0.66666666666666666667");
        assert_eq!(show(d("-1").divide(d("8"))), "-0.125");
        assert_eq!(show(d("9.00").divide(d("3"))), "3.00");
    }

    #[test]
    fn rounding_is_half_to_even() {
        assert_eq!(divide_rounding(5, 2), 2);
        assert_eq!(divide_rounding(7, 2), 4);
        assert_eq!(divide_rounding(-5, 2), -2);
        assert_eq!(divide_rounding(-7, 2), -4);
        assert_eq!(divide_rounding(5, 3), 2);
    }

    #[test]
    fn remainder() {
        assert_eq!(show(d("7").remainder(d("3"))), "1");
        assert_eq!(show(d("15").remainder(d("5"))), "0");
        assert_eq!(show(d("7.5").remainder(d("2"))), "1.5");
        assert_eq!(show(d("-7").remainder(d("3"))), "-1");
    }

    #[test]
    fn dividing_by_zero_is_an_error() {
        assert_eq!(d("1").divide(d("0")).unwrap_err(), MathError::DivideByZero);
        assert_eq!(d("1").remainder(d("0.0")).unwrap_err(), MathError::DivideByZero);
    }

    #[test]
    fn overflow_is_an_error_not_a_panic() {
        let big = Decimal::from_int(i128::MAX);
        assert_eq!(big.add(d("1")).unwrap_err(), MathError::TooLarge);
        assert_eq!(big.multiply(d("2")).unwrap_err(), MathError::TooLarge);
        assert_eq!(Decimal::from_int(i128::MIN).negate().unwrap_err(), MathError::TooLarge);
        assert!(big.divide(d("0.0000001")).is_err());
    }

    #[test]
    fn comparing_ignores_trailing_zeros() {
        assert_eq!(d("0.10"), d("0.1"));
        assert_eq!(d("5"), d("5.000"));
        assert!(d("2.5") < d("10"));
        assert!(d("-1") < d("0"));
        assert!(d("0.3") > d("0.29999"));
    }
}
