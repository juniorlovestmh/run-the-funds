//! CLI handler for `rtf rules {add,list,remove}` (S05).

use std::str::FromStr;

use serde::Serialize;
use uuid::Uuid;

use crate::domain::error::DomainError;
use crate::domain::rules::{MatchField, MatchKind, Rule, RuleRepository};
use crate::infrastructure::storage::{Database, SqliteRuleRepository};

use super::response::{CliResponse, ErrorResponse};

#[derive(Serialize)]
struct RuleView {
    id: String,
    name: String,
    match_field: String,
    match_kind: String,
    match_pattern: String,
    category_id: String,
    priority: i64,
    created_at: String,
    updated_at: String,
}

impl From<&Rule> for RuleView {
    fn from(r: &Rule) -> Self {
        Self {
            id: r.id.clone(),
            name: r.name.clone(),
            match_field: r.match_field.to_string(),
            match_kind: r.match_kind.to_string(),
            match_pattern: r.match_pattern.clone(),
            category_id: r.category_id.clone(),
            priority: r.priority,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

pub fn handle_add(
    db: &Database,
    name: String,
    match_field: String,
    match_kind: Option<String>,
    pattern: String,
    category_id: String,
    priority: Option<i64>,
) {
    match add_rule(db, name, match_field, match_kind, pattern, category_id, priority) {
        Ok(rule) => {
            let view = RuleView::from(&rule);
            let response = CliResponse::ok(view);
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn add_rule(
    db: &Database,
    name: String,
    match_field: String,
    match_kind: Option<String>,
    pattern: String,
    category_id: String,
    priority: Option<i64>,
) -> Result<Rule, DomainError> {
    let field = MatchField::from_str(&match_field).map_err(DomainError::Validation)?;
    let kind = match match_kind.as_deref() {
        None => MatchKind::Substring,
        Some(s) => MatchKind::from_str(s).map_err(DomainError::Validation)?,
    };
    let priority = priority.unwrap_or(100);
    let rule = Rule::new(
        Uuid::new_v4().to_string(),
        name,
        field,
        kind,
        pattern,
        category_id,
        priority,
    )?;
    SqliteRuleRepository::new(db).save(&rule)?;
    Ok(rule)
}

pub fn handle_list(db: &Database, format: String) {
    let repo = SqliteRuleRepository::new(db);
    let rules = match repo.find_all() {
        Ok(r) => r,
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    };
    let views: Vec<RuleView> = rules.iter().map(RuleView::from).collect();
    if format == "json" {
        let response = CliResponse::ok(&views);
        println!("{}", serde_json::to_string_pretty(&response).unwrap());
    } else {
        print_table(&views);
    }
}

fn print_table(views: &[RuleView]) {
    if views.is_empty() {
        println!("No rules. Run `rtf rules add ...` to create one.");
        return;
    }
    println!(
        "{:<36}  {:<8}  {:<12}  {:<10}  {:<30}  {}",
        "ID", "Priority", "Field", "Kind", "Pattern", "Name"
    );
    println!("{}", "-".repeat(120));
    for v in views {
        println!(
            "{:<36}  {:<8}  {:<12}  {:<10}  {:<30}  {}",
            v.id,
            v.priority,
            v.match_field,
            v.match_kind,
            if v.match_pattern.len() > 30 {
                format!("{}…", &v.match_pattern[..29])
            } else {
                v.match_pattern.clone()
            },
            v.name,
        );
    }
}

pub fn handle_remove(db: &Database, id: String) {
    let repo = SqliteRuleRepository::new(db);
    match repo.find_by_id(&id) {
        Ok(Some(r)) => {
            if let Err(e) = repo.delete(&id) {
                print_error(&e.to_string());
                std::process::exit(1);
            }
            let response = CliResponse::ok(serde_json::json!({
                "removed": 1,
                "id": r.id,
                "name": r.name,
            }));
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Ok(None) => {
            print_error(&format!("rule not found: {id}"));
            std::process::exit(1);
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

fn print_error(msg: &str) {
    let response = ErrorResponse::new(msg);
    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_category(db: &Database) {
        let conn = db.conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO category_groups (id, name, created_at) VALUES ('g1','g',?1)",
            [&now],
        )
        .ok();
        conn.execute(
            "INSERT INTO categories (id, group_id, name, created_at) VALUES ('c1','g1','Food',?1)",
            [&now],
        )
        .unwrap();
    }

    #[test]
    fn add_rule_happy_path_defaults_to_substring() {
        let db = Database::in_memory().unwrap();
        seed_category(&db);
        let rule = add_rule(
            &db,
            "Student Loan".into(),
            "payee".into(),
            None,
            "ADVS ED SERV".into(),
            "c1".into(),
            Some(150),
        )
        .unwrap();
        assert_eq!(rule.match_kind, MatchKind::Substring);
        assert_eq!(rule.priority, 150);
    }

    #[test]
    fn add_rule_accepts_regex_kind() {
        let db = Database::in_memory().unwrap();
        seed_category(&db);
        let rule = add_rule(
            &db,
            "Wise".into(),
            "payee".into(),
            Some("regex".into()),
            r"^WISE( INC)?".into(),
            "c1".into(),
            None,
        )
        .unwrap();
        assert_eq!(rule.match_kind, MatchKind::Regex);
        assert_eq!(rule.priority, 100); // default
    }

    #[test]
    fn add_rule_rejects_invalid_regex() {
        let db = Database::in_memory().unwrap();
        seed_category(&db);
        let err = add_rule(
            &db,
            "bad".into(),
            "payee".into(),
            Some("regex".into()),
            "[unclosed".into(),
            "c1".into(),
            None,
        )
        .unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn add_rule_rejects_unknown_match_field() {
        let db = Database::in_memory().unwrap();
        seed_category(&db);
        assert!(add_rule(
            &db,
            "x".into(),
            "bogus".into(),
            None,
            "p".into(),
            "c1".into(),
            None,
        )
        .is_err());
    }

    #[test]
    fn add_rule_rejects_unknown_category_via_fk() {
        let db = Database::in_memory().unwrap();
        // No seeded category → FK failure on save.
        assert!(add_rule(
            &db,
            "x".into(),
            "payee".into(),
            None,
            "p".into(),
            "c-missing".into(),
            None,
        )
        .is_err());
    }

    #[test]
    fn add_rule_amount_pattern_validated() {
        let db = Database::in_memory().unwrap();
        seed_category(&db);
        assert!(add_rule(
            &db,
            "Over $100".into(),
            "amount".into(),
            None,
            ">100".into(),
            "c1".into(),
            None,
        )
        .is_ok());
        // Bogus amount pattern rejected at rule construction.
        assert!(add_rule(
            &db,
            "bad".into(),
            "amount".into(),
            None,
            "not-numeric".into(),
            "c1".into(),
            None,
        )
        .is_err());
    }

    #[test]
    fn list_returns_rules_sorted_by_priority_desc() {
        let db = Database::in_memory().unwrap();
        seed_category(&db);
        add_rule(
            &db,
            "Low".into(),
            "payee".into(),
            None,
            "a".into(),
            "c1".into(),
            Some(10),
        )
        .unwrap();
        add_rule(
            &db,
            "High".into(),
            "payee".into(),
            None,
            "b".into(),
            "c1".into(),
            Some(200),
        )
        .unwrap();
        let rules = SqliteRuleRepository::new(&db).find_all().unwrap();
        assert_eq!(rules[0].name, "High");
        assert_eq!(rules[1].name, "Low");
    }
}
