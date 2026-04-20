pub mod accounts;
pub mod categories;
pub mod categorize;
pub mod connections;
pub mod convert;
pub mod pluggy;
pub mod response;
pub mod rules;
pub mod simplefin;
pub mod spending;
pub mod sync;
pub mod tags;
pub mod teller;
pub mod transactions;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rtf")]
#[command(about = "Personal finance CLI with multi-currency support and agent orchestration")]
#[command(version)]
pub struct Cli {
    /// Path to the database file
    #[arg(long, default_value = "rtf.db")]
    pub db: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage accounts
    Accounts {
        #[command(subcommand)]
        command: AccountCommands,
    },
    /// Manage transactions
    Transactions {
        #[command(subcommand)]
        command: TransactionCommands,
    },
    /// Split a transaction into multiple category allocations
    /// (e.g. a $150 Costco charge → $100 groceries + $50 household).
    /// Each --split is parsed as `category_id:amount[:notes]`.
    /// Split amounts must sum to the parent transaction's absolute amount.
    TransactionSplit {
        /// Parent transaction id
        #[arg(long = "id")]
        transaction_id: String,
        /// A split entry: category_id:amount[:notes]. Use at least twice.
        #[arg(long = "split")]
        split: Vec<String>,
    },
    /// Convert an amount between USD and BRL at a historical PTAX rate
    Convert {
        /// Amount to convert (e.g. 1000, -45.99); leading `-` is allowed
        #[arg(allow_hyphen_values = true)]
        amount: String,
        /// Source currency (USD or BRL)
        from: String,
        /// Target currency (USD or BRL)
        #[arg(long)]
        to: String,
        /// Rate date (YYYY-MM-DD); defaults to today
        #[arg(long)]
        date: Option<String>,
    },
    /// SimpleFIN bank-sync provider management
    Simplefin {
        #[command(subcommand)]
        command: SimplefinCommands,
    },
    /// Pluggy bank-sync provider management
    Pluggy {
        #[command(subcommand)]
        command: PluggyCommands,
    },
    /// Teller bank-sync provider management
    Teller {
        #[command(subcommand)]
        command: TellerCommands,
    },
    /// Manage per-bank provider connections (one row per linked bank)
    Connections {
        #[command(subcommand)]
        command: ConnectionsCommands,
    },
    /// Manage categorization rules (pattern → category)
    Rules {
        #[command(subcommand)]
        command: RulesCommands,
    },
    /// Apply categorization rules, OR detect transfers with --detect-transfers
    Categorize {
        /// Show what would change without writing
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Clear existing category_ids first, then re-apply all rules
        /// (only meaningful without --detect-transfers)
        #[arg(long, default_value_t = false)]
        reset: bool,
        /// Scope to a single account (default: all accounts).
        /// Ignored when --detect-transfers is set (transfer detection is cross-account by design).
        #[arg(long = "account-id")]
        account_id: Option<String>,
        /// Switch mode: pair same-amount debits/credits across accounts (±3 days).
        /// Mutually exclusive with rule-based categorization.
        #[arg(long = "detect-transfers", default_value_t = false)]
        detect_transfers: bool,
    },
    /// Pull new transactions from configured bank-sync providers
    Sync {
        /// Provider to sync: `simplefin` (required for now; unified multi-provider lands later)
        #[arg(long)]
        provider: Option<String>,
        /// Override the sync window start (YYYY-MM-DD); default uses each account's last_sync_at, falling back to a 2-year backfill on first run
        #[arg(long)]
        since: Option<String>,
    },
    /// Manage category groups (top-level buckets like "Essentials", "Wants")
    CategoryGroups {
        #[command(subcommand)]
        command: CategoryGroupsCommands,
    },
    /// Manage categories (leaf buckets like "Groceries" inside a group)
    Categories {
        #[command(subcommand)]
        command: CategoriesCommands,
    },
    /// List tags (read-only; tags are imported on `rtf sync` from Monarch)
    Tags {
        #[command(subcommand)]
        command: TagsCommands,
    },
    /// Produce a spending rollup by category / group, scoped to a date window
    Spending {
        /// Window start (YYYY-MM-DD); default: no lower bound
        #[arg(long)]
        from: Option<String>,
        /// Window end (YYYY-MM-DD, inclusive); default: no upper bound
        #[arg(long)]
        to: Option<String>,
        /// Restrict to a single account id (default: all accounts)
        #[arg(long = "account-id")]
        account_id: Option<String>,
        /// Include transfer pairs in the report (default: excluded)
        #[arg(long = "include-transfers", default_value_t = false)]
        include_transfers: bool,
        /// Output format — "table" or "json"
        #[arg(long, default_value = "table")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum SimplefinCommands {
    /// Exchange a SimpleFIN setup token for an access URL and store it in the local DB
    Setup {
        /// The base64-encoded setup token printed by SimpleFIN
        token: String,
    },
}

#[derive(Subcommand)]
pub enum PluggyCommands {
    /// Save Pluggy per-app credentials (client_id + client_secret) in the local DB.
    /// Run once per Pluggy Application; per-bank items come later via `pluggy connect`.
    Setup {
        #[arg(long = "client-id")]
        client_id: String,
        #[arg(long = "client-secret")]
        client_secret: String,
    },
    /// Link a new bank via Pluggy Connect (opens your default browser).
    /// Run once per bank — each call creates a new item row.
    Connect,
}

#[derive(Subcommand)]
pub enum RulesCommands {
    /// Add a new rule that maps a pattern to a category
    Add {
        /// Human-readable name for the rule (e.g. "Student Loan")
        #[arg(long)]
        name: String,
        /// Which transaction field to match against: payee | description | amount
        #[arg(long = "match-field")]
        match_field: String,
        /// Match kind: substring (default) or regex. Ignored for amount fields.
        #[arg(long = "match-kind")]
        match_kind: Option<String>,
        /// Pattern — substring, regex, or amount DSL (>100 / <=50 / =0 / etc.)
        #[arg(long)]
        pattern: String,
        /// Target category id (from `rtf categories list`)
        #[arg(long = "category-id")]
        category_id: String,
        /// Higher priority wins when multiple rules match; default 100.
        #[arg(long)]
        priority: Option<i64>,
    },
    /// List every stored rule
    List {
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Delete a rule by id
    Remove {
        #[arg(long)]
        id: String,
    },
}

#[derive(Subcommand)]
pub enum ConnectionsCommands {
    /// List every stored bank connection (secrets are not shown)
    List {
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Delete a stored bank connection.
    /// Pass either --id <uuid> OR (--provider <p> --external-id <remote-id>).
    Remove {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long = "external-id")]
        external_id: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum TellerCommands {
    /// Save Teller per-app credentials (app_id, cert, key) in the local DB.
    /// Run once per Teller Application; per-bank enrollments come later via `teller connect`.
    Setup {
        /// Teller Application ID (from your teller.io dashboard)
        #[arg(long = "app-id")]
        app_id: String,
        /// Path to the client certificate PEM (required for Development/Production tiers)
        #[arg(long)]
        cert: Option<String>,
        /// Path to the client private key PEM (required together with --cert)
        #[arg(long)]
        key: Option<String>,
        /// Teller environment: sandbox | development | production.
        /// Defaults to `development` when cert+key are provided, else `sandbox`.
        #[arg(long)]
        environment: Option<String>,
    },
    /// Link a new bank via Teller Connect (opens your default browser).
    /// Run once per bank — each call creates a new enrollment row.
    Connect,
}

#[derive(Subcommand)]
pub enum AccountCommands {
    /// Create a new account
    Create {
        #[arg(long)]
        name: String,
        #[arg(long, value_name = "TYPE")]
        r#type: String,
        #[arg(long)]
        currency: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        institution: Option<String>,
    },
    /// List all accounts
    List {
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Link a local account to a bank-sync provider (simplefin or pluggy)
    Link {
        /// Local account id (UUID)
        #[arg(long)]
        id: String,
        /// Provider name: `simplefin` or `pluggy`
        #[arg(long)]
        provider: String,
        /// Provider-side account id (e.g. SimpleFIN's account.id)
        #[arg(long = "external-id")]
        external_id: String,
        /// Allow overwriting an existing link on this account
        #[arg(long, default_value_t = false)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum TagsCommands {
    /// List all tags
    List {
        #[arg(long, default_value = "table")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum CategoryGroupsCommands {
    /// Create a new category group
    Create {
        #[arg(long)]
        name: String,
    },
    /// List all category groups
    List {
        #[arg(long, default_value = "table")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum CategoriesCommands {
    /// Create a new category under an existing group
    Create {
        #[arg(long)]
        name: String,
        #[arg(long = "group-id")]
        group_id: String,
    },
    /// List all categories
    List {
        #[arg(long, default_value = "table")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum TransactionCommands {
    /// Import transactions from an OFX or CSV file into an existing account
    Import {
        /// Path to the OFX or CSV file to import
        #[arg(long)]
        file: String,
        /// Source format — "ofx" or "csv"
        #[arg(long)]
        format: String,
        /// Destination account id
        #[arg(long)]
        account_id: String,
    },
    /// List transactions, optionally filtered by account
    List {
        /// Filter to a specific account id (default: all accounts)
        #[arg(long)]
        account_id: Option<String>,
        /// Output format — "table" or "json"
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Look up a single transaction by id
    Get {
        #[arg(long)]
        id: String,
    },
}
