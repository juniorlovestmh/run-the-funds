use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CurrencyCode {
    USD,
    BRL,
}

impl fmt::Display for CurrencyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CurrencyCode::USD => write!(f, "USD"),
            CurrencyCode::BRL => write!(f, "BRL"),
        }
    }
}

impl std::str::FromStr for CurrencyCode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "USD" => Ok(CurrencyCode::USD),
            "BRL" => Ok(CurrencyCode::BRL),
            other => Err(format!("unsupported currency: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_usd() {
        assert_eq!(CurrencyCode::USD.to_string(), "USD");
    }

    #[test]
    fn display_brl() {
        assert_eq!(CurrencyCode::BRL.to_string(), "BRL");
    }

    #[test]
    fn parse_uppercase() {
        assert_eq!("USD".parse::<CurrencyCode>().unwrap(), CurrencyCode::USD);
        assert_eq!("BRL".parse::<CurrencyCode>().unwrap(), CurrencyCode::BRL);
    }

    #[test]
    fn parse_lowercase() {
        assert_eq!("usd".parse::<CurrencyCode>().unwrap(), CurrencyCode::USD);
        assert_eq!("brl".parse::<CurrencyCode>().unwrap(), CurrencyCode::BRL);
    }

    #[test]
    fn parse_mixed_case() {
        assert_eq!("Usd".parse::<CurrencyCode>().unwrap(), CurrencyCode::USD);
    }

    #[test]
    fn parse_unsupported_currency_fails() {
        let err = "EUR".parse::<CurrencyCode>().unwrap_err();
        assert!(err.contains("unsupported currency"));
        assert!(err.contains("EUR"));
    }

    #[test]
    fn parse_empty_string_fails() {
        assert!("".parse::<CurrencyCode>().is_err());
    }

    #[test]
    fn equality() {
        assert_eq!(CurrencyCode::USD, CurrencyCode::USD);
        assert_ne!(CurrencyCode::USD, CurrencyCode::BRL);
    }

    #[test]
    fn serde_roundtrip() {
        let code = CurrencyCode::BRL;
        let json = serde_json::to_string(&code).unwrap();
        let deserialized: CurrencyCode = serde_json::from_str(&json).unwrap();
        assert_eq!(code, deserialized);
    }

    #[test]
    fn hash_usable_as_map_key() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(CurrencyCode::USD, "dollar");
        map.insert(CurrencyCode::BRL, "real");
        assert_eq!(map[&CurrencyCode::USD], "dollar");
        assert_eq!(map[&CurrencyCode::BRL], "real");
    }
}
