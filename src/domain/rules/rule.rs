//! Categorization rules (S05).
//!
//! A rule maps a pattern to a category. During `rtf categorize`, each
//! uncategorized transaction is matched against every rule in descending
//! priority order; the first rule whose pattern matches wins and its
//! category_id is assigned to the transaction.
//!
//! **Match kinds:**
//! - `Substring`: case-insensitive substring match on the selected field
//!   (payee or description). Fast and covers most real-world patterns
//!   ("contains ADVS ED SERV" → student loan).
//! - `Regex`: RE2-style regex via the `regex` crate. Compiles at validation
//!   time — invalid patterns are rejected on `Rule::new`, not later during
//!   categorization. Case-insensitive by default (pattern is wrapped with
//!   `(?i)` prefix).
//!
//! **Amount patterns** use a tiny DSL: `>100`, `<50`, `=0`, `>=10.00`,
//! `<=5.99`. `match_kind` is ignored for amount fields.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchField {
    Payee,
    Description,
    Amount,
}

impl fmt::Display for MatchField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Payee => write!(f, "payee"),
            Self::Description => write!(f, "description"),
            Self::Amount => write!(f, "amount"),
        }
    }
}

impl FromStr for MatchField {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "payee" => Ok(Self::Payee),
            "description" => Ok(Self::Description),
            "amount" => Ok(Self::Amount),
            other => Err(format!("unknown match_field: {other} (expected payee|description|amount)")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MatchKind {
    /// Case-insensitive substring match. Default.
    Substring,
    /// RE2 regex (via the `regex` crate). Always case-insensitive.
    Regex,
}

impl fmt::Display for MatchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Substring => write!(f, "substring"),
            Self::Regex => write!(f, "regex"),
        }
    }
}

impl FromStr for MatchKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "substring" => Ok(Self::Substring),
            "regex" => Ok(Self::Regex),
            other => Err(format!("unknown match_kind: {other} (expected substring|regex)")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub match_field: MatchField,
    pub match_kind: MatchKind,
    pub match_pattern: String,
    pub category_id: String,
    pub priority: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Rule {
    pub fn new(
        id: String,
        name: String,
        match_field: MatchField,
        match_kind: MatchKind,
        match_pattern: String,
        category_id: String,
        priority: i64,
    ) -> Result<Self, DomainError> {
        if name.trim().is_empty() {
            return Err(DomainError::Validation("rule name is required".into()));
        }
        if match_pattern.trim().is_empty() {
            return Err(DomainError::Validation("rule match_pattern is required".into()));
        }
        if category_id.trim().is_empty() {
            return Err(DomainError::Validation("rule category_id is required".into()));
        }

        // Pre-validate the pattern shape so bad rules get rejected at
        // save-time, not during the next `categorize` run.
        match match_field {
            MatchField::Amount => {
                parse_amount_pattern(&match_pattern)?;
            }
            MatchField::Payee | MatchField::Description => {
                if match_kind == MatchKind::Regex {
                    // Wrap with (?i) so the stored pattern is what the user
                    // sees, but compilation is always case-insensitive.
                    regex::Regex::new(&format!("(?i){}", match_pattern)).map_err(|e| {
                        DomainError::Validation(format!(
                            "invalid regex `{match_pattern}`: {e}"
                        ))
                    })?;
                }
            }
        }

        let now = Utc::now();
        Ok(Self {
            id,
            name,
            match_field,
            match_kind,
            match_pattern,
            category_id,
            priority,
            created_at: now,
            updated_at: now,
        })
    }

    /// Does this rule match the given transaction?
    pub fn matches(&self, payee: Option<&str>, description: Option<&str>, amount: &rust_decimal::Decimal) -> bool {
        match self.match_field {
            MatchField::Payee => {
                payee.is_some_and(|p| self.text_matches(p))
            }
            MatchField::Description => {
                description.is_some_and(|d| self.text_matches(d))
            }
            MatchField::Amount => {
                // Amount patterns were validated in `new`; panic on bad
                // patterns here would indicate a DB-level corruption.
                parse_amount_pattern(&self.match_pattern)
                    .map(|pat| pat.matches(amount))
                    .unwrap_or(false)
            }
        }
    }

    fn text_matches(&self, haystack: &str) -> bool {
        match self.match_kind {
            MatchKind::Substring => {
                haystack.to_lowercase().contains(&self.match_pattern.to_lowercase())
            }
            MatchKind::Regex => {
                // Re-compile here rather than caching on the struct — rule
                // sets are small (dozens) and compiling once per categorize
                // run would need &mut. If this ever shows up as a hot path,
                // cache a `LazyLock<Vec<Regex>>` on the service.
                regex::Regex::new(&format!("(?i){}", self.match_pattern))
                    .map(|re| re.is_match(haystack))
                    .unwrap_or(false)
            }
        }
    }
}

// ---- Amount pattern DSL -----------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum AmountPattern {
    Gt(rust_decimal::Decimal),
    Gte(rust_decimal::Decimal),
    Lt(rust_decimal::Decimal),
    Lte(rust_decimal::Decimal),
    Eq(rust_decimal::Decimal),
}

impl AmountPattern {
    fn matches(&self, amount: &rust_decimal::Decimal) -> bool {
        match self {
            Self::Gt(x) => amount > x,
            Self::Gte(x) => amount >= x,
            Self::Lt(x) => amount < x,
            Self::Lte(x) => amount <= x,
            Self::Eq(x) => amount == x,
        }
    }
}

fn parse_amount_pattern(pattern: &str) -> Result<AmountPattern, DomainError> {
    let trimmed = pattern.trim();
    // Order matters: check `>=` before `>` and `<=` before `<`.
    let (op, rest) = if let Some(r) = trimmed.strip_prefix(">=") {
        (">=", r)
    } else if let Some(r) = trimmed.strip_prefix("<=") {
        ("<=", r)
    } else if let Some(r) = trimmed.strip_prefix('>') {
        (">", r)
    } else if let Some(r) = trimmed.strip_prefix('<') {
        ("<", r)
    } else if let Some(r) = trimmed.strip_prefix('=') {
        ("=", r)
    } else {
        return Err(DomainError::Validation(format!(
            "amount pattern must start with >, <, =, >=, or <=; got: {pattern}"
        )));
    };
    let value = rust_decimal::Decimal::from_str(rest.trim()).map_err(|e| {
        DomainError::Validation(format!(
            "amount pattern value `{rest}` is not a valid decimal: {e}"
        ))
    })?;
    Ok(match op {
        ">" => AmountPattern::Gt(value),
        ">=" => AmountPattern::Gte(value),
        "<" => AmountPattern::Lt(value),
        "<=" => AmountPattern::Lte(value),
        "=" => AmountPattern::Eq(value),
        _ => unreachable!(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn mk_rule(field: MatchField, kind: MatchKind, pattern: &str) -> Result<Rule, DomainError> {
        Rule::new(
            "r1".into(),
            "test rule".into(),
            field,
            kind,
            pattern.into(),
            "cat-1".into(),
            100,
        )
    }

    #[test]
    fn new_rule_validates_required_fields() {
        assert!(Rule::new(
            "r1".into(), "".into(),
            MatchField::Payee, MatchKind::Substring,
            "x".into(), "cat-1".into(), 100
        ).is_err());
        assert!(mk_rule(MatchField::Payee, MatchKind::Substring, "").is_err());
    }

    #[test]
    fn new_rule_rejects_invalid_regex_at_save_time() {
        let err = mk_rule(MatchField::Payee, MatchKind::Regex, "[unclosed").unwrap_err();
        match err {
            DomainError::Validation(msg) => assert!(msg.contains("invalid regex")),
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn new_rule_accepts_valid_regex() {
        let rule = mk_rule(MatchField::Payee, MatchKind::Regex, r"^WISE( INC)?$").unwrap();
        assert_eq!(rule.match_kind, MatchKind::Regex);
    }

    #[test]
    fn substring_matches_case_insensitive() {
        let rule = mk_rule(MatchField::Payee, MatchKind::Substring, "advs ed serv").unwrap();
        assert!(rule.matches(Some("ADVS ED SERV PPD STUDNTLOAN"), None, &dec!(-100)));
        assert!(!rule.matches(Some("Whole Foods"), None, &dec!(-45)));
    }

    #[test]
    fn substring_returns_false_when_field_absent() {
        let rule = mk_rule(MatchField::Payee, MatchKind::Substring, "anything").unwrap();
        assert!(!rule.matches(None, None, &dec!(-10)));
    }

    #[test]
    fn regex_matches_with_implicit_case_insensitive() {
        let rule = mk_rule(MatchField::Payee, MatchKind::Regex, r"wise( inc)?").unwrap();
        assert!(rule.matches(Some("WISE INC"), None, &dec!(-100)));
        assert!(rule.matches(Some("wise"), None, &dec!(-100)));
        assert!(!rule.matches(Some("Citi"), None, &dec!(-100)));
    }

    #[test]
    fn regex_anchor_semantics() {
        // ^ anchors at the start; case-insensitive applies.
        let rule = mk_rule(MatchField::Payee, MatchKind::Regex, r"^CAPITAL ONE").unwrap();
        assert!(rule.matches(Some("CAPITAL ONE Venture"), None, &dec!(-50)));
        assert!(!rule.matches(Some("some Capital One thing"), None, &dec!(-50)));
    }

    #[test]
    fn description_field_matched_not_payee() {
        let rule = mk_rule(MatchField::Description, MatchKind::Substring, "subscription").unwrap();
        assert!(rule.matches(Some("Netflix"), Some("monthly subscription"), &dec!(-9.99)));
        assert!(!rule.matches(Some("Netflix"), Some("one-time charge"), &dec!(-9.99)));
        // Even if payee matches, description is the field being looked at.
        let rule2 = mk_rule(MatchField::Description, MatchKind::Substring, "netflix").unwrap();
        assert!(!rule2.matches(Some("Netflix"), Some("monthly"), &dec!(-9.99)));
    }

    #[test]
    fn amount_pattern_gt() {
        let rule = mk_rule(MatchField::Amount, MatchKind::Substring, ">100").unwrap();
        assert!(rule.matches(None, None, &dec!(150)));
        assert!(!rule.matches(None, None, &dec!(100)));
        assert!(!rule.matches(None, None, &dec!(50)));
    }

    #[test]
    fn amount_pattern_gte() {
        let rule = mk_rule(MatchField::Amount, MatchKind::Substring, ">=100").unwrap();
        assert!(rule.matches(None, None, &dec!(150)));
        assert!(rule.matches(None, None, &dec!(100)));
        assert!(!rule.matches(None, None, &dec!(99.99)));
    }

    #[test]
    fn amount_pattern_lt_lte() {
        let lt = mk_rule(MatchField::Amount, MatchKind::Substring, "<0").unwrap();
        assert!(lt.matches(None, None, &dec!(-10)));
        assert!(!lt.matches(None, None, &dec!(0)));

        let lte = mk_rule(MatchField::Amount, MatchKind::Substring, "<=0").unwrap();
        assert!(lte.matches(None, None, &dec!(0)));
        assert!(lte.matches(None, None, &dec!(-5)));
        assert!(!lte.matches(None, None, &dec!(0.01)));
    }

    #[test]
    fn amount_pattern_eq() {
        let rule = mk_rule(MatchField::Amount, MatchKind::Substring, "=149.00").unwrap();
        assert!(rule.matches(None, None, &dec!(149.00)));
        assert!(!rule.matches(None, None, &dec!(149.01)));
    }

    #[test]
    fn amount_pattern_rejects_bogus_syntax() {
        assert!(mk_rule(MatchField::Amount, MatchKind::Substring, "just-a-number").is_err());
        assert!(mk_rule(MatchField::Amount, MatchKind::Substring, ">abc").is_err());
        assert!(mk_rule(MatchField::Amount, MatchKind::Substring, "").is_err());
    }

    #[test]
    fn match_field_from_str_accepts_lowercase() {
        assert_eq!(MatchField::from_str("payee").unwrap(), MatchField::Payee);
        assert_eq!(MatchField::from_str("DESCRIPTION").unwrap(), MatchField::Description);
        assert!(MatchField::from_str("bogus").is_err());
    }

    #[test]
    fn match_kind_from_str() {
        assert_eq!(MatchKind::from_str("substring").unwrap(), MatchKind::Substring);
        assert_eq!(MatchKind::from_str("REGEX").unwrap(), MatchKind::Regex);
        assert!(MatchKind::from_str("glob").is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let rule = mk_rule(MatchField::Payee, MatchKind::Regex, r"^WISE").unwrap();
        let json = serde_json::to_string(&rule).unwrap();
        let decoded: Rule = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, rule);
    }
}
