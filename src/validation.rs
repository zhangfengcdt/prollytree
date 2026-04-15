/*
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

//! Input validation for keys and values.
//!
//! Enforces size limits to prevent denial-of-service through oversized inputs.

use thiserror::Error;

/// Maximum allowed key size (64 KB).
pub const MAX_KEY_SIZE: usize = 64 * 1024;

/// Maximum allowed value size (100 MB).
pub const MAX_VALUE_SIZE: usize = 100 * 1024 * 1024;

/// Validation error for key/value inputs.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    #[error("key must not be empty")]
    EmptyKey,

    #[error("key size {0} bytes exceeds maximum of {MAX_KEY_SIZE} bytes")]
    KeyTooLarge(usize),

    #[error("value size {0} bytes exceeds maximum of {MAX_VALUE_SIZE} bytes")]
    ValueTooLarge(usize),
}

/// Validate a key.
///
/// Returns `Ok(())` if the key is non-empty and within [`MAX_KEY_SIZE`].
pub fn validate_key(key: &[u8]) -> Result<(), ValidationError> {
    if key.is_empty() {
        return Err(ValidationError::EmptyKey);
    }
    if key.len() > MAX_KEY_SIZE {
        return Err(ValidationError::KeyTooLarge(key.len()));
    }
    Ok(())
}

/// Validate a value.
///
/// Returns `Ok(())` if the value is within [`MAX_VALUE_SIZE`].
/// Empty values are allowed.
pub fn validate_value(value: &[u8]) -> Result<(), ValidationError> {
    if value.len() > MAX_VALUE_SIZE {
        return Err(ValidationError::ValueTooLarge(value.len()));
    }
    Ok(())
}

/// Validate a key-value pair.
pub fn validate_kv(key: &[u8], value: &[u8]) -> Result<(), ValidationError> {
    validate_key(key)?;
    validate_value(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_key_rejected() {
        assert_eq!(validate_key(b""), Err(ValidationError::EmptyKey));
    }

    #[test]
    fn test_normal_key_accepted() {
        assert!(validate_key(b"hello").is_ok());
    }

    #[test]
    fn test_max_key_accepted() {
        let key = vec![0u8; MAX_KEY_SIZE];
        assert!(validate_key(&key).is_ok());
    }

    #[test]
    fn test_oversized_key_rejected() {
        let key = vec![0u8; MAX_KEY_SIZE + 1];
        assert_eq!(
            validate_key(&key),
            Err(ValidationError::KeyTooLarge(MAX_KEY_SIZE + 1))
        );
    }

    #[test]
    fn test_empty_value_accepted() {
        assert!(validate_value(b"").is_ok());
    }

    #[test]
    fn test_normal_value_accepted() {
        assert!(validate_value(b"some data").is_ok());
    }

    #[test]
    fn test_oversized_value_rejected() {
        let value = vec![0u8; MAX_VALUE_SIZE + 1];
        assert_eq!(
            validate_value(&value),
            Err(ValidationError::ValueTooLarge(MAX_VALUE_SIZE + 1))
        );
    }

    #[test]
    fn test_validate_kv_checks_both() {
        assert_eq!(validate_kv(b"", b"val"), Err(ValidationError::EmptyKey));
        let big = vec![0u8; MAX_VALUE_SIZE + 1];
        assert_eq!(
            validate_kv(b"key", &big),
            Err(ValidationError::ValueTooLarge(MAX_VALUE_SIZE + 1))
        );
        assert!(validate_kv(b"key", b"val").is_ok());
    }
}
