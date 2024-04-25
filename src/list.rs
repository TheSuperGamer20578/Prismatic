use std::path::PathBuf;
use anyhow::Result;
use crate::mod_locator::{Manifest, Mod, UpdateKeys};

pub fn list(mods: &Vec<Mod>, mods_dir: &PathBuf) -> Result<()> {
    for Mod {path, manifest: Manifest {name, description, author, version, update_keys, ..}, ..} in mods {
        let relative_path = path.strip_prefix(mods_dir)?;
        if let Some(name) = name {
            println!("{name} ({})", relative_path.display());
        } else {
            println!("{}", relative_path.display());
        }
        if let Some(description) = description {
            println!("{description}");
        }
        if let Some(author) = author {
            println!("Author: {author}");
        }
        if let Some(version) = version {
            println!("Version: {version}");
        }
        if let Some(source) = update_keys.preferred() {
            println!("Source: {}", source.source);
        }
        println!();
    }
    println!("{} mods found", mods.len());
    Ok(())
}
