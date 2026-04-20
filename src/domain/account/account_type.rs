use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Checking,
    Savings,
    CreditCard,
    Loan,
    Brokerage,
    Other,
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountType::Checking => write!(f, "checking"),
            AccountType::Savings => write!(f, "savings"),
            AccountType::CreditCard => write!(f, "credit_card"),
            AccountType::Loan => write!(f, "loan"),
            AccountType::Brokerage => write!(f, "brokerage"),
            AccountType::Other => write!(f, "other"),
        }
    }
}

impl std::str::FromStr for AccountType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "checking" => Ok(AccountType::Checking),
            "savings" => Ok(AccountType::Savings),
            "credit_card" | "creditcard" | "credit-card" => Ok(AccountType::CreditCard),
            "loan" => Ok(AccountType::Loan),
            "brokerage" | "investment" | "roth" | "ira" | "crypto" | "cryptocurrency" => {
                Ok(AccountType::Brokerage)
            }
            "other" | "vehicle" | "car" | "real_estate" => Ok(AccountType::Other),
            other => Err(format!("unknown account type: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_all_variants() {
        assert_eq!(AccountType::Checking.to_string(), "checking");
        assert_eq!(AccountType::Savings.to_string(), "savings");
        assert_eq!(AccountType::CreditCard.to_string(), "credit_card");
        assert_eq!(AccountType::Loan.to_string(), "loan");
    }

    #[test]
    fn parse_standard_forms() {
        assert_eq!(
            "checking".parse::<AccountType>().unwrap(),
            AccountType::Checking
        );
        assert_eq!(
            "savings".parse::<AccountType>().unwrap(),
            AccountType::Savings
        );
        assert_eq!(
            "credit_card".parse::<AccountType>().unwrap(),
            AccountType::CreditCard
        );
        assert_eq!("loan".parse::<AccountType>().unwrap(), AccountType::Loan);
    }

    #[test]
    fn parse_credit_card_variants() {
        assert_eq!(
            "creditcard".parse::<AccountType>().unwrap(),
            AccountType::CreditCard
        );
        assert_eq!(
            "credit-card".parse::<AccountType>().unwrap(),
            AccountType::CreditCard
        );
        assert_eq!(
            "CREDIT_CARD".parse::<AccountType>().unwrap(),
            AccountType::CreditCard
        );
    }

    #[test]
    fn parse_case_insensitive() {
        assert_eq!(
            "CHECKING".parse::<AccountType>().unwrap(),
            AccountType::Checking
        );
        assert_eq!(
            "Savings".parse::<AccountType>().unwrap(),
            AccountType::Savings
        );
    }

    #[test]
    fn parse_unknown_fails() {
        let err = "flarglebargle".parse::<AccountType>().unwrap_err();
        assert!(err.contains("unknown account type"));
    }

    #[test]
    fn serde_roundtrip() {
        for variant in [
            AccountType::Checking,
            AccountType::Savings,
            AccountType::CreditCard,
            AccountType::Loan,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: AccountType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn serde_uses_snake_case() {
        let json = serde_json::to_value(AccountType::CreditCard).unwrap();
        assert_eq!(json, "credit_card");
    }
}
