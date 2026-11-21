use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use serde::{Deserialize, Serialize};

pub const MINIMUM_LENGTH: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SealedPasscode(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasscodeProblem {
    TooShort,
    Mismatch,
    WrongCurrent,
    Unavailable,
}

impl SealedPasscode {
    pub fn seal(secret: &str) -> Result<Self, PasscodeProblem> {
        if secret.chars().count() < MINIMUM_LENGTH {
            return Err(PasscodeProblem::TooShort);
        }
        Argon2::default()
            .hash_password(secret.as_bytes())
            .map(|hash| Self(hash.to_string()))
            .map_err(|_| PasscodeProblem::Unavailable)
    }

    pub fn matches(&self, secret: &str) -> bool {
        PasswordHash::new(&self.0).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(secret.as_bytes(), &hash)
                .is_ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{PasscodeProblem, SealedPasscode};

    #[test]
    fn verifies_the_right_secret_only() {
        let sealed = SealedPasscode::seal("2468").unwrap();
        assert!(sealed.matches("2468"));
        assert!(!sealed.matches("2469"));
        assert!(!sealed.matches(""));
    }

    #[test]
    fn never_stores_the_secret() {
        let sealed = SealedPasscode::seal("secret-code").unwrap();
        assert!(!sealed.0.contains("secret-code"));
        assert!(sealed.0.starts_with("$argon2id$"));
    }

    #[test]
    fn salts_every_seal() {
        assert_ne!(
            SealedPasscode::seal("1234").unwrap(),
            SealedPasscode::seal("1234").unwrap()
        );
    }

    #[test]
    fn rejects_short_secrets() {
        assert_eq!(SealedPasscode::seal("123"), Err(PasscodeProblem::TooShort));
    }
}
