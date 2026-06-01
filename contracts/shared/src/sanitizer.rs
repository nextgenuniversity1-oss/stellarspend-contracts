extern crate alloc;
use crate::errors::SharedError;
use soroban_sdk::{Env, String};

/// Validates that a description string only contains safe, printable ASCII characters.
/// Allows alphanumeric characters, spaces, and basic punctuation.
pub fn sanitize_description(env: &Env, description: &String) -> Result<(), SharedError> {
    if description.len() > 256 {
        return Err(SharedError::InvalidLength);
    }

    let mut bytes = [0u8; 256];
    let len = description.len() as usize;
    description.copy_into_slice(&mut bytes[..len]);

    for b in bytes.iter().take(len) {
        let is_valid = match *b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => true,
            b' ' | b'.' | b',' | b'-' | b'_' | b'!' | b'?' | b'\'' => true,
            _ => false,
        };

        if !is_valid {
            return Err(SharedError::InvalidInput);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn accepts_valid_descriptions() {
        let env = Env::default();
        let valid_cases = [
            "Valid transfer description",
            "Payment for service!",
            "Invoice_123-A.",
            "Are you sure?",
            "It's a test",
        ];

        for text in valid_cases.iter() {
            let s = String::from_str(&env, text);
            assert_eq!(sanitize_description(&env, &s), Ok(()));
        }
    }

    #[test]
    fn rejects_invalid_characters() {
        let env = Env::default();
        let invalid_cases = [
            "Invalid\nnewline",
            "Invalid\ttab",
            "HTML <script>alert(1)</script>",
            "Symbols like @ or #",
            "Emoji 🔥",
        ];

        for text in invalid_cases.iter() {
            let s = String::from_str(&env, text);
            assert_eq!(
                sanitize_description(&env, &s),
                Err(SharedError::InvalidInput)
            );
        }
    }

    #[test]
    fn rejects_too_long() {
        let env = Env::default();
        // 257 characters
        let mut long_text = alloc::string::String::new();
        for _ in 0..257 {
            long_text.push('A');
        }
        let s = String::from_str(&env, &long_text);
        assert_eq!(
            sanitize_description(&env, &s),
            Err(SharedError::InvalidLength)
        );
    }
}
