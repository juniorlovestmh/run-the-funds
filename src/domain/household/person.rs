use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Relationship {
    // `Self` is a reserved keyword in Rust; the trailing underscore is a
    // language workaround. The canonical wire/storage form is "self" — we
    // override serde's default rename to drop the underscore so JSON
    // serialization, the Display impl, and the SQLite text encoding all agree.
    #[serde(rename = "self")]
    Self_,
    Spouse,
    Child,
    Other,
}

impl std::fmt::Display for Relationship {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Relationship::Self_ => write!(f, "self"),
            Relationship::Spouse => write!(f, "spouse"),
            Relationship::Child => write!(f, "child"),
            Relationship::Other => write!(f, "other"),
        }
    }
}

impl std::str::FromStr for Relationship {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "self" => Ok(Relationship::Self_),
            "spouse" => Ok(Relationship::Spouse),
            "child" => Ok(Relationship::Child),
            "other" => Ok(Relationship::Other),
            unknown => Err(format!("unknown relationship: {unknown}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub relationship: Relationship,
    pub created_at: DateTime<Utc>,
}

impl Person {
    pub fn new(id: String, name: String, relationship: Relationship) -> Result<Self, DomainError> {
        if name.trim().is_empty() {
            return Err(DomainError::Validation("person name is required".into()));
        }
        Ok(Self {
            id,
            name,
            relationship,
            created_at: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_person() {
        let p = Person::new("p-001".into(), "Sky".into(), Relationship::Self_).unwrap();
        assert_eq!(p.id, "p-001");
        assert_eq!(p.name, "Sky");
        assert_eq!(p.relationship, Relationship::Self_);
        assert!(p.created_at <= Utc::now());
    }

    #[test]
    fn new_rejects_empty_name() {
        let result = Person::new("p-001".into(), "".into(), Relationship::Self_);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("name is required"));
    }

    #[test]
    fn new_rejects_whitespace_name() {
        assert!(Person::new("p-001".into(), "   ".into(), Relationship::Self_).is_err());
    }

    #[test]
    fn all_relationship_variants() {
        Person::new("1".into(), "Me".into(), Relationship::Self_).unwrap();
        Person::new("2".into(), "Wife".into(), Relationship::Spouse).unwrap();
        Person::new("3".into(), "Baby".into(), Relationship::Child).unwrap();
        Person::new("4".into(), "Other".into(), Relationship::Other).unwrap();
    }

    #[test]
    fn relationship_display() {
        assert_eq!(Relationship::Self_.to_string(), "self");
        assert_eq!(Relationship::Spouse.to_string(), "spouse");
        assert_eq!(Relationship::Child.to_string(), "child");
        assert_eq!(Relationship::Other.to_string(), "other");
    }

    #[test]
    fn relationship_parse() {
        assert_eq!("self".parse::<Relationship>().unwrap(), Relationship::Self_);
        assert_eq!(
            "spouse".parse::<Relationship>().unwrap(),
            Relationship::Spouse
        );
        assert_eq!(
            "CHILD".parse::<Relationship>().unwrap(),
            Relationship::Child
        );
        assert_eq!(
            "Other".parse::<Relationship>().unwrap(),
            Relationship::Other
        );
    }

    #[test]
    fn relationship_parse_unknown_fails() {
        assert!("friend".parse::<Relationship>().is_err());
    }

    #[test]
    fn serde_roundtrip() {
        let p = Person::new("p-001".into(), "Sky".into(), Relationship::Self_).unwrap();
        let json = serde_json::to_string(&p).unwrap();
        let deserialized: Person = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, p.id);
        assert_eq!(deserialized.name, p.name);
        assert_eq!(deserialized.relationship, p.relationship);
    }

    #[test]
    fn serde_relationship_snake_case() {
        let json = serde_json::to_value(Relationship::Self_).unwrap();
        assert_eq!(json, "self");
    }

    #[test]
    fn serde_and_text_encoding_agree() {
        // The serde representation and the Display/FromStr text encoding
        // (used by the SQLite repo) must produce the same canonical string
        // for every variant — otherwise JSON output and DB storage will
        // diverge the moment they meet.
        for variant in [
            Relationship::Self_,
            Relationship::Spouse,
            Relationship::Child,
            Relationship::Other,
        ] {
            let serde_form = serde_json::to_value(&variant)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
            let display_form = variant.to_string();
            assert_eq!(
                serde_form, display_form,
                "serde and Display disagree for {variant:?}: {serde_form:?} vs {display_form:?}",
            );

            // And round-trip the shared string back through FromStr to
            // confirm both sides can parse the same canonical form.
            let parsed: Relationship = display_form.parse().unwrap();
            assert_eq!(parsed, variant);
        }
    }
}
