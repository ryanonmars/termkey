use clap::Parser;

use termkey::cli::{Cli, Commands};
use termkey::crypto::secure;
use termkey::error::{Result, TermKeyError};
use termkey::{apply_configured_vault_dir_override, commands, repl, ui, vault};

fn main() {
    if let Err(error) = run() {
        ui::borders::print_error(&error.to_string());
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    secure::harden_process().map_err(TermKeyError::Io)?;
    termkey::initialize_storage_paths()?;
    vault::storage::migrate_vault_if_needed();

    if cli.command.is_some() {
        ui::setup_app_theme(true);
    }
    apply_configured_vault_dir_override();

    match cli.command {
        None => repl::run(),
        Some(cmd) => match cmd {
            Commands::Init => commands::init::run(),
            Commands::Add => commands::add::run(),
            Commands::List { ref filter } => commands::list::run(filter.as_deref()),
            Commands::View { ref name } => commands::view::run(name),
            Commands::Edit { ref name } => commands::edit::run(name),
            Commands::Rename {
                ref old_name,
                ref new_name,
            } => commands::rename::run(old_name, new_name),
            Commands::Delete { ref name } => commands::delete::run(name),
            Commands::Copy { ref name } => commands::copy::run(name),
            Commands::Search { ref query } => commands::search::run(query),
            Commands::Export { ref directory } => commands::export::run(directory),
            Commands::Import { ref file } => commands::import::run(file),
            Commands::Passwd => commands::passwd::run(),
            Commands::Recover => commands::recover::run(),
            Commands::Update => commands::update_cmd::run(),
            Commands::Reset { yes } => commands::reset::run(yes),
            Commands::Uninstall { yes } => commands::uninstall::run(yes),
            Commands::Config {
                show,
                ref clipboard_timeout,
            } => commands::config_cmd::run(show, *clipboard_timeout),
            Commands::Derive { ref name } => commands::derive::run(name),
        },
    }
}
