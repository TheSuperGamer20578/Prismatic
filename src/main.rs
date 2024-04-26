#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::panic)]
#![cfg_attr(not(debug_assertions), forbid(clippy::dbg_macro))]

mod mod_locator;
mod list;
mod update;

use std::env::home_dir;
use std::path::PathBuf;
use clap::{Parser, Subcommand};
use anyhow::{anyhow, bail, Result};
use crate::list::list;
use crate::mod_locator::locate_mods;
use crate::update::update;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    #[clap(long, short = 'd')]
    mods_dir: Option<PathBuf>,
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Add {
        source: String,
    },
    Remove {
        id: String,
    },
    List,
    Update {
        #[clap(long, short)]
        force: bool,
    },
}

#[tokio::main]
async fn main() {
    if let Err(err) = main_inner().await {
        eprintln!("Error: {err}");
        #[cfg(debug_assertions)] {
            eprintln!("{err:?}");
        }
    }
}

async fn main_inner() -> Result<()> {
    let args = Args::parse();
    let mods_dir = args.mods_dir.ok_or(()).or_else(|()|
        if cfg!(unix) {Ok(home_dir().unwrap().join(".steam/steam/steamapps/common/Stardew Valley/Mods"))}
        else if cfg!(windows) {Ok(PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\Stardew Valley\Mods"))}
        else {Err(anyhow!("Could not determine mods directory, please specify with -d <dir>"))}
    )?;
    if !mods_dir.is_dir() {
        bail!("Invalid mods directory");
    }
    let mods = locate_mods(&mods_dir).await?;
    match args.command {
        Command::Add { .. } => bail!("Not yet implemented"),
        Command::Remove { .. } => bail!("Not yet implemented"),
        Command::List => list(&mods, &mods_dir)?,
        Command::Update { force } => update(&mods, &mods_dir, force).await?,
    }
    Ok(())
}
