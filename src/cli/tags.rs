//! `rtf tags list` (S06 T01). Read-only — Monarch is the source of
//! truth for tag creation; rtf mirrors during sync.

use serde::Serialize;

use crate::domain::tag::{Tag, TagRepository};
use crate::infrastructure::storage::{Database, SqliteTagRepository};

use super::response::{CliResponse, ErrorResponse};

#[derive(Serialize)]
struct TagView {
    id: String,
    name: String,
    color: Option<String>,
    external_id: Option<String>,
    external_provider: Option<String>,
}

impl From<&Tag> for TagView {
    fn from(t: &Tag) -> Self {
        Self {
            id: t.id.clone(),
            name: t.name.clone(),
            color: t.color.clone(),
            external_id: t.external_id.clone(),
            external_provider: t.external_provider.clone(),
        }
    }
}

pub fn handle_list(db: &Database, format: String) {
    let repo = SqliteTagRepository::new(db);
    let tags = match repo.find_all() {
        Ok(t) => t,
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    };
    let views: Vec<TagView> = tags.iter().map(TagView::from).collect();

    if format == "json" {
        let response = CliResponse::ok(&views);
        println!("{}", serde_json::to_string_pretty(&response).unwrap());
    } else if views.is_empty() {
        println!("No tags yet. Tags are imported on `rtf sync` from Monarch.");
    } else {
        println!(
            "{:<36}  {:<30}  {:<10}  {}",
            "ID", "Name", "Color", "Source"
        );
        println!("{}", "-".repeat(100));
        for v in views {
            let color = v.color.as_deref().unwrap_or("-");
            let source = match (v.external_provider.as_deref(), v.external_id.as_deref()) {
                (Some(p), Some(eid)) => format!("{p}:{eid}"),
                _ => "local".into(),
            };
            println!("{:<36}  {:<30}  {:<10}  {}", v.id, v.name, color, source);
        }
    }
}

fn print_error(msg: &str) {
    let response = ErrorResponse::new(msg);
    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
}
