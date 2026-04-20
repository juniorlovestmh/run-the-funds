use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("validation error: {0}")]
    Validation(String),

    #[error("not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },

    #[error("duplicate: {entity} already exists: {detail}")]
    Duplicate { entity: String, detail: String },

    #[error("currency mismatch: expected {expected}, got {got}")]
    CurrencyMismatch { expected: String, got: String },

    #[error("import error: {0}")]
    Import(String),

    #[error("storage error: {0}")]
    Storage(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_error_message() {
        let err = DomainError::Validation("name is required".into());
        assert_eq!(err.to_string(), "validation error: name is required");
    }

    #[test]
    fn not_found_error_message() {
        let err = DomainError::NotFound {
            entity: "Account".into(),
            id: "abc-123".into(),
        };
        assert_eq!(err.to_string(), "not found: Account with id abc-123");
    }

    #[test]
    fn duplicate_error_message() {
        let err = DomainError::Duplicate {
            entity: "Account".into(),
            detail: "name 'Nubank Checking'".into(),
        };
        assert!(err.to_string().contains("Nubank Checking"));
    }

    #[test]
    fn currency_mismatch_error_message() {
        let err = DomainError::CurrencyMismatch {
            expected: "USD".into(),
            got: "BRL".into(),
        };
        assert_eq!(
            err.to_string(),
            "currency mismatch: expected USD, got BRL"
        );
    }

    #[test]
    fn storage_error_message() {
        let err = DomainError::Storage("connection failed".into());
        assert_eq!(err.to_string(), "storage error: connection failed");
    }

    #[test]
    fn import_error_message() {
        let err = DomainError::Import("ofx: bad FITID".into());
        assert_eq!(err.to_string(), "import error: ofx: bad FITID");
    }

    #[test]
    fn error_is_send_and_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<DomainError>();
        assert_sync::<DomainError>();
    }
}
