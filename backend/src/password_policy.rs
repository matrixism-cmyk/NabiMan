//! What a new or changed NabiMan password must be: at least 12 characters
//! (the same rule install.sh sets for the first administrator; a Korean
//! letter counts as one) and at most the 72 bytes bcrypt reads. Passwords
//! already stored keep working; the rule applies when one is chosen.

pub const MIN_CHARS: usize = 12;
pub const MAX_BYTES: usize = 72;

pub const RULE: &str = "Password must be at least 12 characters (at most 72 bytes)";
pub const RULE_KO: &str = "비밀번호는 12자 이상으로 입력하세요 (최대 72바이트)";

pub fn acceptable(password: &str) -> bool {
    password.chars().count() >= MIN_CHARS && password.len() <= MAX_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_characters_to_seventy_two_bytes() {
        assert!(acceptable("abcdefgh1234"));
        assert!(!acceptable("abcdefgh123"));
        assert!(acceptable("비밀번호는열두글자입니다"), "twelve Korean letters are twelve characters");
        assert!(!acceptable("비밀번호열한글자입니다"));
        assert!(acceptable(&"a".repeat(72)) && !acceptable(&"a".repeat(73)));
    }
}
