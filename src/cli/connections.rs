//! CLI handler for `rtf connections list [--format json|table]` (S04C).
//!
//! Read-only view over the `provider_connections` table for operator
//! inspection. Never exposes the per-connection `data` blob (which holds
//! secrets like access_tokens) — only public metadata: id, provider,
//! external_id, institution_name, created_at.

use serde::Serialize;

use crate::domain::connections::{ProviderConnection, ProviderConnectionRepository};
use crate::infrastructure::storage::{Database, SqliteProviderConnectionRepository};

#[cfg(test)]
use crate::domain::error::DomainError;

use super::response::{CliResponse, ErrorResponse};

#[derive(Serialize)]
struct ConnectionView {
    id: String,
    provider: String,
    external_id: String,
    institution_name: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<&ProviderConnection> for ConnectionView {
    fn from(c: &ProviderConnection) -> Self {
        Self {
            id: c.id.clone(),
            provider: c.provider.clone(),
            external_id: c.external_id.clone(),
            institution_name: c.institution_name.clone(),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

pub fn handle_list(db: &Database, format: String) {
    let repo = SqliteProviderConnectionRepository::new(db);
    let connections = match repo.find_all() {
        Ok(c) => c,
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    };

    let views: Vec<ConnectionView> = connections.iter().map(ConnectionView::from).collect();

    if format == "json" {
        let response = CliResponse::ok(&views);
        println!("{}", serde_json::to_string_pretty(&response).unwrap());
    } else {
        print_table(&views);
    }
}

pub fn handle_remove(
    db: &Database,
    id: Option<String>,
    provider: Option<String>,
    external_id: Option<String>,
) {
    let repo = SqliteProviderConnectionRepository::new(db);

    // Validate mutually-exclusive arg groups.
    let found = match resolve_target(&repo, id, provider, external_id) {
        Ok(c) => c,
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    };

    if let Err(e) = repo.delete(&found.id) {
        print_error(&e.to_string());
        std::process::exit(1);
    }

    let response = CliResponse::ok(serde_json::json!({
        "removed": 1,
        "id": found.id,
        "provider": found.provider,
        "external_id": found.external_id,
        "institution_name": found.institution_name,
    }));
    println!("{}", serde_json::to_string_pretty(&response).unwrap());
}

fn resolve_target(
    repo: &SqliteProviderConnectionRepository,
    id: Option<String>,
    provider: Option<String>,
    external_id: Option<String>,
) -> Result<ProviderConnection, crate::domain::error::DomainError> {
    use crate::domain::error::DomainError;

    match (id, provider, external_id) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => Err(DomainError::Validation(
            "pass either --id OR (--provider + --external-id), not both".into(),
        )),
        (Some(id), None, None) => repo.find_by_id(&id)?.ok_or_else(|| DomainError::NotFound {
            entity: "provider_connection".into(),
            id,
        }),
        (None, Some(provider), Some(ext)) => {
            repo.find_by_external_id(&provider, &ext)?
                .ok_or_else(|| DomainError::NotFound {
                    entity: "provider_connection".into(),
                    id: format!("{provider}:{ext}"),
                })
        }
        (None, None, None) => Err(DomainError::Validation(
            "specify --id OR (--provider + --external-id)".into(),
        )),
        (None, Some(_), None) | (None, None, Some(_)) => Err(DomainError::Validation(
            "--provider and --external-id must be provided together".into(),
        )),
    }
}

fn print_table(views: &[ConnectionView]) {
    if views.is_empty() {
        println!(
            "No bank connections. Run `rtf teller connect` or `rtf pluggy connect` to link a bank."
        );
        return;
    }
    println!(
        "{:<36}  {:<10}  {:<30}  {:<25}  {}",
        "ID", "Provider", "Institution", "External ID", "Created"
    );
    println!("{}", "-".repeat(120));
    for v in views {
        println!(
            "{:<36}  {:<10}  {:<30}  {:<25}  {}",
            v.id,
            v.provider,
            v.institution_name.as_deref().unwrap_or("-"),
            // Truncate long external_ids so the table stays readable.
            if v.external_id.len() > 25 {
                format!("{}…", &v.external_id[..24])
            } else {
                v.external_id.clone()
            },
            v.created_at,
        );
    }
}

fn print_error(msg: &str) {
    let response = ErrorResponse::new(msg);
    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn seed_connection(db: &Database, provider: &str, ext: &str, institution: &str) -> String {
        let c = ProviderConnection::new(
            Uuid::new_v4().to_string(),
            provider.into(),
            ext.into(),
            "{}".into(),
            Some(institution.into()),
        )
        .unwrap();
        SqliteProviderConnectionRepository::new(db)
            .save(&c)
            .unwrap();
        c.id
    }

    #[test]
    fn resolve_target_by_id_happy_path() {
        let db = Database::in_memory().unwrap();
        let id = seed_connection(&db, "teller", "enr_1", "Chase");
        let repo = SqliteProviderConnectionRepository::new(&db);
        let found = resolve_target(&repo, Some(id.clone()), None, None).unwrap();
        assert_eq!(found.id, id);
    }

    #[test]
    fn resolve_target_by_provider_and_external_id() {
        let db = Database::in_memory().unwrap();
        seed_connection(&db, "teller", "enr_1", "Chase");
        let repo = SqliteProviderConnectionRepository::new(&db);
        let found =
            resolve_target(&repo, None, Some("teller".into()), Some("enr_1".into())).unwrap();
        assert_eq!(found.external_id, "enr_1");
    }

    #[test]
    fn resolve_target_not_found_by_id() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderConnectionRepository::new(&db);
        let err = resolve_target(&repo, Some("missing".into()), None, None).unwrap_err();
        assert!(matches!(err, DomainError::NotFound { .. }));
    }

    #[test]
    fn resolve_target_not_found_by_provider_external_id() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderConnectionRepository::new(&db);
        let err =
            resolve_target(&repo, None, Some("teller".into()), Some("missing".into())).unwrap_err();
        assert!(matches!(err, DomainError::NotFound { .. }));
    }

    #[test]
    fn resolve_target_rejects_both_arg_groups() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderConnectionRepository::new(&db);
        let err = resolve_target(
            &repo,
            Some("x".into()),
            Some("teller".into()),
            Some("y".into()),
        )
        .unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn resolve_target_rejects_no_args() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderConnectionRepository::new(&db);
        let err = resolve_target(&repo, None, None, None).unwrap_err();
        assert!(matches!(err, DomainError::Validation(_)));
    }

    #[test]
    fn resolve_target_rejects_partial_provider_pair() {
        let db = Database::in_memory().unwrap();
        let repo = SqliteProviderConnectionRepository::new(&db);
        assert!(resolve_target(&repo, None, Some("teller".into()), None).is_err());
        assert!(resolve_target(&repo, None, None, Some("x".into())).is_err());
    }
}
