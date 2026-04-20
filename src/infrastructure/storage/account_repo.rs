use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::domain::account::{Account, AccountRepository, AccountType};
use crate::domain::currency::{CurrencyCode, Money};
use crate::domain::error::DomainError;

use super::database::Database;

pub struct SqliteAccountRepository<'a> {
    db: &'a Database,
}

impl<'a> SqliteAccountRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    fn row_to_account(row: &rusqlite::Row) -> rusqlite::Result<Account> {
        let id: String = row.get("id")?;
        let name: String = row.get("name")?;
        let account_type_str: String = row.get("account_type")?;
        let currency_str: String = row.get("currency")?;
        let owner: String = row.get("owner")?;
        let institution: Option<String> = row.get("institution")?;
        let account_number_last4: Option<String> = row.get("account_number_last4")?;
        let balance_amount_str: String = row.get("balance_amount")?;
        let balance_currency_str: String = row.get("balance_currency")?;
        let credit_limit_amount: Option<String> = row.get("credit_limit_amount")?;
        let credit_limit_currency: Option<String> = row.get("credit_limit_currency")?;
        let interest_rate_str: Option<String> = row.get("interest_rate")?;
        let notes: Option<String> = row.get("notes")?;
        let external_provider: Option<String> = row.get("external_provider")?;
        let external_account_id: Option<String> = row.get("external_account_id")?;
        let last_sync_at_str: Option<String> = row.get("last_sync_at")?;
        let created_at_str: String = row.get("created_at")?;
        let updated_at_str: String = row.get("updated_at")?;

        let account_type = AccountType::from_str(&account_type_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let currency = CurrencyCode::from_str(&currency_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let balance_amount = Decimal::from_str(&balance_amount_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::from(e))
        })?;
        let balance_currency = CurrencyCode::from_str(&balance_currency_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::from(e))
        })?;

        let credit_limit = match (credit_limit_amount, credit_limit_currency) {
            (Some(amt), Some(cur)) => {
                let amount = Decimal::from_str(&amt).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        9,
                        rusqlite::types::Type::Text,
                        Box::from(e),
                    )
                })?;
                let currency = CurrencyCode::from_str(&cur).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        10,
                        rusqlite::types::Type::Text,
                        Box::from(e),
                    )
                })?;
                Some(Money::new(amount, currency))
            }
            _ => None,
        };

        let interest_rate = interest_rate_str
            .map(|s| Decimal::from_str(&s))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    11,
                    rusqlite::types::Type::Text,
                    Box::from(e),
                )
            })?;

        let parse_dt = |s: &str, col: usize| {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        col,
                        rusqlite::types::Type::Text,
                        Box::from(e),
                    )
                })
        };
        let created_at = parse_dt(&created_at_str, 12)?;
        let updated_at = parse_dt(&updated_at_str, 13)?;
        let last_sync_at = last_sync_at_str
            .as_deref()
            .map(|s| parse_dt(s, 14))
            .transpose()?;

        Ok(Account {
            id,
            name,
            account_type,
            currency,
            owner,
            institution,
            account_number_last4,
            balance: Money::new(balance_amount, balance_currency),
            credit_limit,
            interest_rate,
            notes,
            external_provider,
            external_account_id,
            last_sync_at,
            created_at,
            updated_at,
        })
    }
}

impl AccountRepository for SqliteAccountRepository<'_> {
    fn save(&self, account: &Account) -> Result<(), DomainError> {
        // ON CONFLICT(id) DO UPDATE (not INSERT OR REPLACE) so that the
        // partial unique index on (external_provider, external_account_id)
        // actually rejects duplicate-linkage attempts instead of silently
        // replacing the conflicting row.
        self.db
            .conn()
            .execute(
                "INSERT INTO accounts (
                    id, name, account_type, currency, owner, institution,
                    account_number_last4, balance_amount, balance_currency,
                    credit_limit_amount, credit_limit_currency, interest_rate,
                    notes, external_provider, external_account_id, last_sync_at,
                    created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    account_type = excluded.account_type,
                    currency = excluded.currency,
                    owner = excluded.owner,
                    institution = excluded.institution,
                    account_number_last4 = excluded.account_number_last4,
                    balance_amount = excluded.balance_amount,
                    balance_currency = excluded.balance_currency,
                    credit_limit_amount = excluded.credit_limit_amount,
                    credit_limit_currency = excluded.credit_limit_currency,
                    interest_rate = excluded.interest_rate,
                    notes = excluded.notes,
                    external_provider = excluded.external_provider,
                    external_account_id = excluded.external_account_id,
                    last_sync_at = excluded.last_sync_at,
                    updated_at = excluded.updated_at",
                rusqlite::params![
                    account.id,
                    account.name,
                    account.account_type.to_string(),
                    account.currency.to_string(),
                    account.owner,
                    account.institution,
                    account.account_number_last4,
                    account.balance.amount.to_string(),
                    account.balance.currency.to_string(),
                    account.credit_limit.as_ref().map(|m| m.amount.to_string()),
                    account.credit_limit.as_ref().map(|m| m.currency.to_string()),
                    account.interest_rate.map(|r| r.to_string()),
                    account.notes,
                    account.external_provider,
                    account.external_account_id,
                    account.last_sync_at.map(|dt| dt.to_rfc3339()),
                    account.created_at.to_rfc3339(),
                    account.updated_at.to_rfc3339(),
                ],
            )
            .map_err(|e| DomainError::Storage(format!("save account: {e}")))?;
        Ok(())
    }

    fn find_by_id(&self, id: &str) -> Result<Option<Account>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM accounts WHERE id = ?1")
            .map_err(|e| DomainError::Storage(format!("prepare find_by_id: {e}")))?
            .query_row([id], Self::row_to_account)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_id: {e}")))
    }

    fn find_all(&self) -> Result<Vec<Account>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM accounts ORDER BY name")
            .map_err(|e| DomainError::Storage(format!("prepare find_all: {e}")))?
            .query_map([], Self::row_to_account)
            .map_err(|e| DomainError::Storage(format!("find_all: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("find_all collect: {e}")))
    }

    fn find_by_external_link(
        &self,
        provider: &str,
        external_account_id: &str,
    ) -> Result<Option<Account>, DomainError> {
        self.db
            .conn()
            .prepare(
                "SELECT * FROM accounts \
                 WHERE external_provider = ?1 AND external_account_id = ?2 LIMIT 1",
            )
            .map_err(|e| DomainError::Storage(format!("prepare find_by_external_link: {e}")))?
            .query_row([provider, external_account_id], Self::row_to_account)
            .optional()
            .map_err(|e| DomainError::Storage(format!("find_by_external_link: {e}")))
    }

    fn find_by_provider(&self, provider: &str) -> Result<Vec<Account>, DomainError> {
        self.db
            .conn()
            .prepare("SELECT * FROM accounts WHERE external_provider = ?1 ORDER BY name")
            .map_err(|e| DomainError::Storage(format!("prepare find_by_provider: {e}")))?
            .query_map([provider], Self::row_to_account)
            .map_err(|e| DomainError::Storage(format!("find_by_provider: {e}")))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DomainError::Storage(format!("find_by_provider collect: {e}")))
    }

    fn delete(&self, id: &str) -> Result<(), DomainError> {
        self.db
            .conn()
            .execute("DELETE FROM accounts WHERE id = ?1", [id])
            .map_err(|e| DomainError::Storage(format!("delete account: {e}")))?;
        Ok(())
    }
}

use rusqlite::OptionalExtension;

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn setup() -> Database {
        Database::in_memory().unwrap()
    }

    fn make_account(id: &str, name: &str, currency: CurrencyCode) -> Account {
        Account::new(
            id.into(),
            name.into(),
            AccountType::Checking,
            currency,
            "Sky".into(),
        )
        .unwrap()
    }

    #[test]
    fn save_and_find_by_id() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);
        let acc = make_account("acc-001", "Nubank Checking", CurrencyCode::BRL);

        repo.save(&acc).unwrap();
        let found = repo.find_by_id("acc-001").unwrap().unwrap();

        assert_eq!(found.id, "acc-001");
        assert_eq!(found.name, "Nubank Checking");
        assert_eq!(found.account_type, AccountType::Checking);
        assert_eq!(found.currency, CurrencyCode::BRL);
        assert_eq!(found.owner, "Sky");
        assert!(found.balance.is_zero());
        assert_eq!(found.balance.currency, CurrencyCode::BRL);
    }

    #[test]
    fn find_by_id_not_found() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);
        assert!(repo.find_by_id("nonexistent").unwrap().is_none());
    }

    #[test]
    fn find_all_empty() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);
        assert!(repo.find_all().unwrap().is_empty());
    }

    #[test]
    fn find_all_returns_sorted_by_name() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);

        repo.save(&make_account("acc-c", "Chase", CurrencyCode::USD))
            .unwrap();
        repo.save(&make_account("acc-a", "Aidvantage", CurrencyCode::USD))
            .unwrap();
        repo.save(&make_account("acc-n", "Nubank", CurrencyCode::BRL))
            .unwrap();

        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].name, "Aidvantage");
        assert_eq!(all[1].name, "Chase");
        assert_eq!(all[2].name, "Nubank");
    }

    #[test]
    fn save_upserts_on_conflict() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);

        let mut acc = make_account("acc-001", "Old Name", CurrencyCode::USD);
        repo.save(&acc).unwrap();

        acc.name = "New Name".into();
        repo.save(&acc).unwrap();

        let found = repo.find_by_id("acc-001").unwrap().unwrap();
        assert_eq!(found.name, "New Name");
        assert_eq!(repo.find_all().unwrap().len(), 1);
    }

    #[test]
    fn delete_removes_account() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);

        repo.save(&make_account("acc-001", "Test", CurrencyCode::USD))
            .unwrap();
        assert!(repo.find_by_id("acc-001").unwrap().is_some());

        repo.delete("acc-001").unwrap();
        assert!(repo.find_by_id("acc-001").unwrap().is_none());
    }

    #[test]
    fn roundtrip_preserves_money_precision() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);

        let mut acc = make_account("acc-001", "Test", CurrencyCode::USD);
        acc.balance = Money::new(dec!(12345.67), CurrencyCode::USD);
        repo.save(&acc).unwrap();

        let found = repo.find_by_id("acc-001").unwrap().unwrap();
        assert_eq!(found.balance.amount, dec!(12345.67));
    }

    #[test]
    fn roundtrip_preserves_optional_fields() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);

        let mut acc = Account::new(
            "cc-001".into(),
            "Capital One".into(),
            AccountType::CreditCard,
            CurrencyCode::USD,
            "Sky".into(),
        )
        .unwrap();
        acc.institution = Some("Capital One".into());
        acc.account_number_last4 = Some("4321".into());
        acc.credit_limit = Some(Money::new(dec!(5000.00), CurrencyCode::USD));
        acc.interest_rate = Some(dec!(24.99));
        acc.notes = Some("Primary credit card".into());

        repo.save(&acc).unwrap();
        let found = repo.find_by_id("cc-001").unwrap().unwrap();

        assert_eq!(found.institution.as_deref(), Some("Capital One"));
        assert_eq!(found.account_number_last4.as_deref(), Some("4321"));
        assert_eq!(found.credit_limit.unwrap().amount, dec!(5000.00));
        assert_eq!(found.interest_rate.unwrap(), dec!(24.99));
        assert_eq!(found.notes.as_deref(), Some("Primary credit card"));
    }

    #[test]
    fn roundtrip_preserves_timestamps() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);
        let acc = make_account("acc-001", "Test", CurrencyCode::USD);
        let original_created = acc.created_at;

        repo.save(&acc).unwrap();
        let found = repo.find_by_id("acc-001").unwrap().unwrap();

        let diff = (found.created_at - original_created)
            .num_milliseconds()
            .abs();
        assert!(diff < 1000, "timestamp drift: {diff}ms");
    }

    #[test]
    fn multiple_currencies() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);

        repo.save(&make_account("usd-001", "Chase", CurrencyCode::USD))
            .unwrap();
        repo.save(&make_account("brl-001", "Nubank", CurrencyCode::BRL))
            .unwrap();

        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 2);

        let currencies: Vec<CurrencyCode> = all.iter().map(|a| a.currency).collect();
        assert!(currencies.contains(&CurrencyCode::USD));
        assert!(currencies.contains(&CurrencyCode::BRL));
    }

    #[test]
    fn roundtrip_preserves_external_link_and_last_sync_at() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);
        let mut acc = make_account("acc-001", "Chase", CurrencyCode::USD);
        acc.link("simplefin".into(), "ext-999".into()).unwrap();
        let ts = Utc::now();
        acc.mark_synced(ts);

        repo.save(&acc).unwrap();
        let found = repo.find_by_id("acc-001").unwrap().unwrap();
        assert_eq!(found.external_provider.as_deref(), Some("simplefin"));
        assert_eq!(found.external_account_id.as_deref(), Some("ext-999"));
        assert!(found.last_sync_at.is_some());
        let diff = (found.last_sync_at.unwrap() - ts).num_milliseconds().abs();
        assert!(diff < 1000);
    }

    #[test]
    fn find_by_external_link_returns_matching_account() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);

        let mut acc = make_account("acc-001", "Chase", CurrencyCode::USD);
        acc.link("simplefin".into(), "ext-abc".into()).unwrap();
        repo.save(&acc).unwrap();

        let found = repo
            .find_by_external_link("simplefin", "ext-abc")
            .unwrap()
            .unwrap();
        assert_eq!(found.id, "acc-001");

        assert!(
            repo.find_by_external_link("simplefin", "nope")
                .unwrap()
                .is_none()
        );
        assert!(
            repo.find_by_external_link("pluggy", "ext-abc")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn find_by_provider_filters_and_ignores_unlinked() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);

        let mut simplefin_a = make_account("sf-1", "Chase", CurrencyCode::USD);
        simplefin_a
            .link("simplefin".into(), "ext-1".into())
            .unwrap();
        let mut simplefin_b = make_account("sf-2", "Capital One", CurrencyCode::USD);
        simplefin_b
            .link("simplefin".into(), "ext-2".into())
            .unwrap();
        let mut pluggy_a = make_account("pl-1", "Nubank", CurrencyCode::BRL);
        pluggy_a.link("pluggy".into(), "ext-3".into()).unwrap();
        let unlinked = make_account("unlinked-1", "Manual", CurrencyCode::USD);

        repo.save(&simplefin_a).unwrap();
        repo.save(&simplefin_b).unwrap();
        repo.save(&pluggy_a).unwrap();
        repo.save(&unlinked).unwrap();

        let sf = repo.find_by_provider("simplefin").unwrap();
        assert_eq!(sf.len(), 2);
        let pg = repo.find_by_provider("pluggy").unwrap();
        assert_eq!(pg.len(), 1);
        assert_eq!(pg[0].id, "pl-1");
        assert!(repo.find_by_provider("nonexistent").unwrap().is_empty());
    }

    #[test]
    fn partial_unique_index_rejects_duplicate_external_link() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);

        let mut a = make_account("a", "Chase A", CurrencyCode::USD);
        a.link("simplefin".into(), "ext-dup".into()).unwrap();
        repo.save(&a).unwrap();

        let mut b = make_account("b", "Chase B", CurrencyCode::USD);
        b.link("simplefin".into(), "ext-dup".into()).unwrap();
        let err = repo.save(&b).unwrap_err();
        assert!(matches!(err, DomainError::Storage(_)));
    }

    #[test]
    fn partial_unique_index_permits_multiple_unlinked_accounts() {
        let db = setup();
        let repo = SqliteAccountRepository::new(&db);

        // Two unlinked accounts (NULL external_provider) should coexist.
        repo.save(&make_account("a", "Manual A", CurrencyCode::USD))
            .unwrap();
        repo.save(&make_account("b", "Manual B", CurrencyCode::USD))
            .unwrap();
        assert_eq!(repo.find_all().unwrap().len(), 2);
    }
}
