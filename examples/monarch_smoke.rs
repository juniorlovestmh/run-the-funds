//! Live smoke against Monarch. Requires `mmoney auth login` already run.
//! Run with: cargo run --example monarch_smoke

use chrono::NaiveDate;
use rtf::infrastructure::sync_adapter::MonarchAdapter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let adapter = MonarchAdapter::subprocess();

    let accounts = adapter.fetch_accounts()?;
    println!("accounts: {}", accounts.len());

    let groups = adapter.fetch_category_groups()?;
    println!("category groups: {}", groups.len());

    let categories = adapter.fetch_categories()?;
    println!("categories: {}", categories.len());

    let tags = adapter.fetch_tags()?;
    println!("tags: {}", tags.len());

    let txns = adapter.fetch_transactions(
        NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 4, 20).unwrap(),
    )?;
    println!("transactions (Apr 1-20, 2026): {}", txns.len());

    if let Some(first) = txns.first() {
        println!(
            "  first: {} {} {} {} cat={:?} tags={:?}",
            first.date,
            first.amount,
            first.currency,
            first.merchant_name.as_deref().unwrap_or("?"),
            first.category_external_id,
            first.tag_external_ids
        );
    }

    Ok(())
}
