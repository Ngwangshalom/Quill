//! Chance tools: `random`, `pick from`, `hash`, and `seed`.

use rand::rngs::{OsRng, StdRng};
use rand::{Rng, RngCore, SeedableRng};
use sha2::{Digest, Sha256};

use crate::decimal::Decimal;
use crate::diagnostic::{Diagnostic, Span};
use crate::value::Value;

/// The run's source of chance: OS entropy, or a fixed seed after `seed`.
#[derive(Default)]
pub enum Chance {
    #[default]
    Open,
    Seeded(Box<StdRng>),
}

impl Chance {
    /// Fix later `random` and `pick` draws to follow from `n`.
    pub fn seed(&mut self, n: i128) {
        let mut bytes = [0u8; 32];
        let raw = n.to_le_bytes();
        for (i, byte) in raw.iter().cycle().take(32).enumerate() {
            bytes[i] = *byte;
        }
        *self = Chance::Seeded(Box::new(StdRng::from_seed(bytes)));
    }

    fn rng(&mut self) -> &mut StdRng {
        if matches!(self, Chance::Open) {
            let mut seed = [0u8; 32];
            OsRng.fill_bytes(&mut seed);
            *self = Chance::Seeded(Box::new(StdRng::from_seed(seed)));
        }
        match self {
            Chance::Seeded(rng) => rng,
            Chance::Open => unreachable!(),
        }
    }

    /// A whole number from `low` through `high`, both ends included.
    pub fn random(
        &mut self,
        low: &Decimal,
        high: &Decimal,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let low = whole(low, "The start of a random range", span)?;
        let high = whole(high, "The end of a random range", span)?;
        if high < low {
            return Err(Diagnostic::new(
                span,
                format!(
                    "random needs the smaller number first, but {low}..{high} goes the other way.\n\
                     Write the smallest, then .., then the largest, like random 1..100."
                ),
            ));
        }
        let width = high
            .checked_sub(low)
            .and_then(|d| d.checked_add(1))
            .filter(|&w| w > 0)
            .ok_or_else(|| {
                Diagnostic::new(
                    span,
                    "That random range is too wide for Quill to draw from.\n\
                     Use a smaller span between the two ends.",
                )
            })?;
        let Ok(width_u) = u128::try_from(width) else {
            return Err(Diagnostic::new(
                span,
                "That random range is too wide for Quill to draw from.\n\
                 Use a smaller span between the two ends.",
            ));
        };
        let offset = self.rng().gen_range(0..width_u);
        let offset = i128::try_from(offset).map_err(|_| {
            Diagnostic::new(
                span,
                "That random range is too wide for Quill to draw from.\n\
                 Use a smaller span between the two ends.",
            )
        })?;
        let value = low.checked_add(offset).ok_or_else(|| {
            Diagnostic::new(
                span,
                "That random range is too wide for Quill to draw from.\n\
                 Use a smaller span between the two ends.",
            )
        })?;
        Ok(Value::Number(Decimal::from_int(value)))
    }

    /// One item from a non-empty list.
    pub fn pick_from(&mut self, list: Value, span: Span) -> Result<Value, Diagnostic> {
        let Value::List(items) = list else {
            return Err(Diagnostic::new(
                span,
                format!(
                    "pick from needs a list, but got {}.\n\
                     pick from names chooses one name from the list.",
                    list.written()
                ),
            ));
        };
        if items.is_empty() {
            return Err(Diagnostic::new(
                span,
                "pick from needs at least one item in the list.\n\
                 An empty list has nothing to choose.",
            ));
        }
        let index = self.rng().gen_range(0..items.len());
        Ok(items[index].clone())
    }
}

fn whole(number: &Decimal, role: &str, span: Span) -> Result<i128, Diagnostic> {
    number.whole().ok_or_else(|| {
        Diagnostic::new(
            span,
            format!(
                "{role} has to be a whole number, but got {}.\n\
                 random 1..100 draws a whole number from 1 through 100.",
                number
            ),
        )
    })
}

/// SHA-256 of `text` as 64 lowercase hex letters.
pub fn hash_text(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_makes_random_repeatable() {
        let mut a = Chance::default();
        a.seed(1);
        let mut b = Chance::default();
        b.seed(1);
        let low = Decimal::from_int(1);
        let high = Decimal::from_int(10);
        let span = Span::default();
        assert_eq!(
            a.random(&low, &high, span).unwrap(),
            b.random(&low, &high, span).unwrap()
        );
    }

    #[test]
    fn hash_is_sha256_hex() {
        let hex = hash_text("desk");
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(hash_text("desk"), hex);
    }
}
