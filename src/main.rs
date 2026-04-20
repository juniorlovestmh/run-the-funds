use clap::Parser;
use rtf::cli::{
    AccountCommands, CategoriesCommands, CategoryGroupsCommands, Cli, Commands,
    ConnectionsCommands, PluggyCommands, RulesCommands, SimplefinCommands, TagsCommands,
    TellerCommands, TransactionCommands,
};
use rtf::infrastructure::storage::Database;

fn main() {
    let cli = Cli::parse();

    let db = match Database::open(&cli.db) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("{{\"status\":\"error\",\"message\":\"failed to open database: {e}\"}}");
            std::process::exit(1);
        }
    };

    match cli.command {
        Commands::Accounts { command } => match command {
            AccountCommands::Create {
                name,
                r#type,
                currency,
                owner,
                institution,
            } => {
                rtf::cli::accounts::handle_create(
                    &db,
                    name,
                    r#type,
                    currency,
                    owner,
                    institution,
                );
            }
            AccountCommands::List { format } => {
                rtf::cli::accounts::handle_list(&db, format);
            }
            AccountCommands::Link {
                id,
                provider,
                external_id,
                force,
            } => {
                rtf::cli::accounts::handle_link(&db, id, provider, external_id, force);
            }
        },
        Commands::Transactions { command } => match command {
            TransactionCommands::Import {
                file,
                format,
                account_id,
            } => {
                rtf::cli::transactions::handle_import(&db, file, format, account_id);
            }
            TransactionCommands::List { account_id, format } => {
                rtf::cli::transactions::handle_list(&db, account_id, format);
            }
            TransactionCommands::Get { id } => {
                rtf::cli::transactions::handle_get(&db, id);
            }
        },
        Commands::Convert {
            amount,
            from,
            to,
            date,
        } => {
            rtf::cli::convert::handle_convert(&db, amount, from, to, date);
        }
        Commands::Simplefin { command } => match command {
            SimplefinCommands::Setup { token } => {
                rtf::cli::simplefin::handle_setup(&db, token);
            }
        },
        Commands::Pluggy { command } => match command {
            PluggyCommands::Setup {
                client_id,
                client_secret,
            } => {
                rtf::cli::pluggy::handle_setup(&db, client_id, client_secret);
            }
            PluggyCommands::Connect => {
                rtf::cli::pluggy::handle_connect(&db);
            }
        },
        Commands::Teller { command } => match command {
            TellerCommands::Setup {
                app_id,
                cert,
                key,
                environment,
            } => {
                rtf::cli::teller::handle_setup(&db, app_id, cert, key, environment);
            }
            TellerCommands::Connect => {
                rtf::cli::teller::handle_connect(&db);
            }
        },
        Commands::Connections { command } => match command {
            ConnectionsCommands::List { format } => {
                rtf::cli::connections::handle_list(&db, format);
            }
            ConnectionsCommands::Remove {
                id,
                provider,
                external_id,
            } => {
                rtf::cli::connections::handle_remove(&db, id, provider, external_id);
            }
        },
        Commands::Sync { provider, since } => {
            rtf::cli::sync::handle_sync(&db, provider, since);
        }
        Commands::Rules { command } => match command {
            RulesCommands::Add {
                name,
                match_field,
                match_kind,
                pattern,
                category_id,
                priority,
            } => {
                rtf::cli::rules::handle_add(
                    &db,
                    name,
                    match_field,
                    match_kind,
                    pattern,
                    category_id,
                    priority,
                );
            }
            RulesCommands::List { format } => {
                rtf::cli::rules::handle_list(&db, format);
            }
            RulesCommands::Remove { id } => {
                rtf::cli::rules::handle_remove(&db, id);
            }
        },
        Commands::Categorize {
            dry_run,
            reset,
            account_id,
            detect_transfers,
        } => {
            rtf::cli::categorize::handle_categorize(
                &db,
                dry_run,
                reset,
                account_id,
                detect_transfers,
            );
        }
        Commands::TransactionSplit {
            transaction_id,
            split,
        } => {
            rtf::cli::transactions::handle_split(&db, transaction_id, split);
        }
        Commands::CategoryGroups { command } => match command {
            CategoryGroupsCommands::Create { name } => {
                rtf::cli::categories::handle_group_create(&db, name);
            }
            CategoryGroupsCommands::List { format } => {
                rtf::cli::categories::handle_group_list(&db, format);
            }
        },
        Commands::Categories { command } => match command {
            CategoriesCommands::Create { name, group_id } => {
                rtf::cli::categories::handle_category_create(&db, name, group_id);
            }
            CategoriesCommands::List { format } => {
                rtf::cli::categories::handle_category_list(&db, format);
            }
        },
        Commands::Spending {
            from,
            to,
            account_id,
            include_transfers,
            format,
        } => {
            rtf::cli::spending::handle_spending(
                &db,
                from,
                to,
                account_id,
                include_transfers,
                format,
            );
        }
        Commands::Tags { command } => match command {
            TagsCommands::List { format } => rtf::cli::tags::handle_list(&db, format),
        },
    }
}
