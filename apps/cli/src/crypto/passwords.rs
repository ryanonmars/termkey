use rand::{rngs::OsRng, seq::SliceRandom, Rng};

const DEFAULT_PASSWORD_LENGTH: usize = 24;
const UPPERCASE: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
const LOWERCASE: &[u8] = b"abcdefghijkmnopqrstuvwxyz";
const DIGITS: &[u8] = b"23456789";
const SYMBOLS: &[u8] = b"!@#$%^&*-_=+?";

pub fn generate_password() -> String {
    generate_password_with_rng(&mut OsRng, DEFAULT_PASSWORD_LENGTH)
}

fn generate_password_with_rng<R: Rng + ?Sized>(rng: &mut R, length: usize) -> String {
    let length = length.max(4);
    let required_sets = [UPPERCASE, LOWERCASE, DIGITS, SYMBOLS];
    let all_chars: Vec<u8> = required_sets.into_iter().flatten().copied().collect();

    let mut password = Vec::with_capacity(length);
    for set in required_sets {
        let idx = rng.gen_range(0..set.len());
        password.push(set[idx]);
    }

    for _ in password.len()..length {
        let idx = rng.gen_range(0..all_chars.len());
        password.push(all_chars[idx]);
    }

    password.shuffle(rng);
    String::from_utf8(password).expect("password characters are valid ASCII")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_any_from(s: &str, chars: &[u8]) -> bool {
        s.as_bytes().iter().any(|ch| chars.contains(ch))
    }

    #[test]
    fn generated_password_has_expected_length() {
        let mut rng = rand::rngs::mock::StepRng::new(1, 1);
        let password = generate_password_with_rng(&mut rng, 24);
        assert_eq!(password.len(), 24);
    }

    #[test]
    fn generated_password_contains_each_required_character_class() {
        let mut rng = rand::rngs::mock::StepRng::new(2, 3);
        let password = generate_password_with_rng(&mut rng, 24);

        assert!(has_any_from(&password, UPPERCASE));
        assert!(has_any_from(&password, LOWERCASE));
        assert!(has_any_from(&password, DIGITS));
        assert!(has_any_from(&password, SYMBOLS));
    }

    #[test]
    fn generated_password_uses_only_allowed_characters() {
        let mut rng = rand::rngs::mock::StepRng::new(5, 7);
        let password = generate_password_with_rng(&mut rng, 24);
        let allowed: Vec<u8> = [UPPERCASE, LOWERCASE, DIGITS, SYMBOLS]
            .into_iter()
            .flatten()
            .copied()
            .collect();

        assert!(password.as_bytes().iter().all(|ch| allowed.contains(ch)));
    }

    #[test]
    fn password_length_is_never_shorter_than_required_classes() {
        let mut rng = rand::rngs::mock::StepRng::new(9, 11);
        let password = generate_password_with_rng(&mut rng, 2);
        assert_eq!(password.len(), 4);
    }
}
