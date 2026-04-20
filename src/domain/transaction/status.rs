use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Pending,
    Cleared,
    Reconciled,
}

impl std::fmt::Display for TransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactionStatus::Pending => write!(f, "pending"),
            TransactionStatus::Cleared => write!(f, "cleared"),
            TransactionStatus::Reconciled => write!(f, "reconciled"),
        }
    }
}

impl std::str::FromStr for TransactionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(TransactionStatus::Pending),
            "cleared" => Ok(TransactionStatus::Cleared),
            "reconciled" => Ok(TransactionStatus::Reconciled),
            other => Err(format!("unknown transaction status: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_all_variants() {
        assert_eq!(TransactionStatus::Pending.to_string(), "pending");
        assert_eq!(TransactionStatus::Cleared.to_string(), "cleared");
        assert_eq!(TransactionStatus::Reconciled.to_string(), "reconciled");
    }

    #[test]
    fn parse_all_variants() {
        assert_eq!("pending".parse::<TransactionStatus>().unwrap(), TransactionStatus::Pending);
        assert_eq!("cleared".parse::<TransactionStatus>().unwrap(), TransactionStatus::Cleared);
        assert_eq!("reconciled".parse::<TransactionStatus>().unwrap(), TransactionStatus::Reconciled);
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!("PENDING".parse::<TransactionStatus>().unwrap(), TransactionStatus::Pending);
    }

    #[test]
    fn parse_unknown_fails() {
        assert!("voided".parse::<TransactionStatus>().is_err());
    }

    #[test]
    fn serde_roundtrip() {
        for variant in [TransactionStatus::Pending, TransactionStatus::Cleared, TransactionStatus::Reconciled] {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: TransactionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }
}
