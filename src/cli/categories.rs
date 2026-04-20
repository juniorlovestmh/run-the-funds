//! CLI for `rtf categories {create,list}` and `rtf category-groups {create,list}` (S05 T05).

use serde::Serialize;
use uuid::Uuid;

use crate::domain::category::{Category, CategoryGroup, CategoryRepository};
use crate::infrastructure::storage::{Database, SqliteCategoryRepository};

use super::response::{CliResponse, ErrorResponse};

#[derive(Serialize)]
struct CategoryView {
    id: String,
    name: String,
    group_id: String,
    created_at: String,
}

impl From<&Category> for CategoryView {
    fn from(c: &Category) -> Self {
        Self {
            id: c.id.clone(),
            name: c.name.clone(),
            group_id: c.group_id.clone(),
            created_at: c.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize)]
struct GroupView {
    id: String,
    name: String,
    created_at: String,
}

impl From<&CategoryGroup> for GroupView {
    fn from(g: &CategoryGroup) -> Self {
        Self {
            id: g.id.clone(),
            name: g.name.clone(),
            created_at: g.created_at.to_rfc3339(),
        }
    }
}

// ---- category-groups -------------------------------------------------------

pub fn handle_group_create(db: &Database, name: String) {
    if name.trim().is_empty() {
        print_error("--name is required");
        std::process::exit(1);
    }
    let repo = SqliteCategoryRepository::new(db);
    let group = CategoryGroup {
        id: Uuid::new_v4().to_string(),
        name,
        created_at: chrono::Utc::now(),
    };
    match repo.save_group(&group) {
        Ok(()) => {
            let response = CliResponse::ok(GroupView::from(&group));
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

pub fn handle_group_list(db: &Database, format: String) {
    let repo = SqliteCategoryRepository::new(db);
    let groups = match repo.find_all_groups() {
        Ok(g) => g,
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    };
    let views: Vec<GroupView> = groups.iter().map(GroupView::from).collect();
    if format == "json" {
        let response = CliResponse::ok(&views);
        println!("{}", serde_json::to_string_pretty(&response).unwrap());
    } else if views.is_empty() {
        println!("No category groups. Run `rtf category-groups create --name <name>` to add one.");
    } else {
        println!("{:<36}  {:<30}  {}", "ID", "Name", "Created");
        println!("{}", "-".repeat(90));
        for v in views {
            println!("{:<36}  {:<30}  {}", v.id, v.name, v.created_at);
        }
    }
}

// ---- categories ------------------------------------------------------------

pub fn handle_category_create(db: &Database, name: String, group_id: String) {
    if name.trim().is_empty() {
        print_error("--name is required");
        std::process::exit(1);
    }
    if group_id.trim().is_empty() {
        print_error("--group-id is required");
        std::process::exit(1);
    }
    let repo = SqliteCategoryRepository::new(db);
    let cat = Category {
        id: Uuid::new_v4().to_string(),
        group_id,
        name,
        created_at: chrono::Utc::now(),
    };
    match repo.save_category(&cat) {
        Ok(()) => {
            let response = CliResponse::ok(CategoryView::from(&cat));
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    }
}

pub fn handle_category_list(db: &Database, format: String) {
    let repo = SqliteCategoryRepository::new(db);
    let categories = match repo.find_all_categories() {
        Ok(c) => c,
        Err(e) => {
            print_error(&e.to_string());
            std::process::exit(1);
        }
    };
    let views: Vec<CategoryView> = categories.iter().map(CategoryView::from).collect();
    if format == "json" {
        let response = CliResponse::ok(&views);
        println!("{}", serde_json::to_string_pretty(&response).unwrap());
    } else if views.is_empty() {
        println!("No categories. Run `rtf categories create --name <name> --group-id <group>` to add one.");
    } else {
        println!("{:<36}  {:<30}  {:<36}  {}", "ID", "Name", "Group ID", "Created");
        println!("{}", "-".repeat(120));
        for v in views {
            println!(
                "{:<36}  {:<30}  {:<36}  {}",
                v.id, v.name, v.group_id, v.created_at
            );
        }
    }
}

fn print_error(msg: &str) {
    let response = ErrorResponse::new(msg);
    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
}
