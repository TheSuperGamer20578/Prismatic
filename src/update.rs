use std::collections::HashSet;
use std::io::Cursor;
use std::path::{PathBuf};
use anyhow::{anyhow, bail, Result};
use async_recursion::async_recursion;
use bytes::Bytes;
#[cfg(feature = "nexus")]
use scraper::{Html, Selector};
use serde_json::json;
use tokio::fs;
use tokio::sync::OnceCell;
use tokio::task::JoinSet;
use zip::ZipArchive;
use crate::mod_locator::{Mod, UpdateKey, UpdateKeys};

#[cfg(feature = "nexus")]
static NEXUS_AUTH: OnceCell<String> = OnceCell::const_new();
static REQWEST: OnceCell<reqwest::Client> = OnceCell::const_new();

async fn reqwest_client() -> &'static reqwest::Client {
    REQWEST.get_or_init(|| async { reqwest::Client::new() }).await
}

pub async fn update(mods: &Vec<Mod>, mods_dir: &PathBuf, force: bool) -> Result<()> {
    let mut updated = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut set: JoinSet<Result<(Mod, UpdateResult)>> = JoinSet::new();
    for mod_ in mods {
        let name = mod_.manifest.name.clone().unwrap_or(mod_.path.strip_prefix(mods_dir)?.display().to_string());
        if let Some(source) = mod_.manifest.update_keys.preferred() {
            if source.subkey.is_some() {
                failed += 1;
                eprintln!("{name}: Subkeys are not yet supported");
                continue;
            }
            match &*source.source.to_lowercase() {
                #[cfg(feature = "github")] "github" => {set.spawn(github(mod_.clone(), source.clone(), force));}
                #[cfg(feature = "nexus")] "nexus" => {set.spawn(nexus(mod_.clone(), source.clone(), force));}
                _ => unreachable!()
            }
        } else {
            failed += 1;
            eprintln!("{name}: Unknown or unsupported source");
        }
    }
    while let Some(res) = set.join_next().await {
        let (mod_, result) = res??;
        let name = mod_.manifest.name.unwrap_or(mod_.path.strip_prefix(mods_dir)?.display().to_string());
        match result {
            UpdateResult::Success { new_version } => {
                updated += 1;
                if let Some(old_version) = mod_.manifest.version {
                    println!("{name}: {old_version} -> {new_version}");
                } else {
                    println!("{name}: {new_version}");
                }
            }
            UpdateResult::AlreadyUpToDate => {
                skipped += 1;
                eprintln!("{name}: Already up to date");
            }
            UpdateResult::Failure(message) => {
                failed += 1;
                eprintln!("{name}: {message}");
            }
        }
    }
    if force {
        eprintln!("{updated} updated, {failed} failed");
    } else {
        eprintln!("{updated} updated, {failed} failed, {skipped} already up to date");
    }
    Ok(())
}

enum UpdateResult {
    Success {
        new_version: String,
    },
    AlreadyUpToDate,
    Failure(String),
}

async fn check_for_updates(mod_: &Mod) -> Result<bool> {
    let Some(key) = mod_.manifest.update_keys.preferred() else {bail!("No preferred update key")};
    Ok(reqwest_client().await
        .post("https://smapi.io/api/v3.0/mods")
        .json(&json!(
            {
                "mods": [
                    {
                        "id": mod_.manifest.unique_id.clone().unwrap_or(format!("FAKE.{}.{}", key.source, key.id)),
                        "updateKeys": [ format!("{}:{}", key.source, key.id) ],
                        "installedVersion": mod_.manifest.version.clone().unwrap_or_default(),
                    },
                ],
                "apiVersion": "4.0.7",  // TODO: Use actual version
            }
        ))
        .send().await?
        .error_for_status()?
        .json::<serde_json::Value>().await?
        .as_array().ok_or(anyhow!("Expected array"))?[0]
        .as_object().ok_or(anyhow!("Expected object"))?
        .contains_key("suggestedUpdate")
    )
}

#[cfg(feature = "github")]
async fn github(mod_: Mod, update_key: UpdateKey, force: bool) -> Result<(Mod, UpdateResult)> {
    if !force && !check_for_updates(&mod_).await? {
        return Ok((mod_, UpdateResult::AlreadyUpToDate));
    }
    let (owner, repo) = update_key.id.split_once('/').ok_or(anyhow!("Invalid GitHub repo"))?;
    let release = octocrab::instance()
        .repos(owner, repo)
        .releases()
        .get_latest().await?;
    let version = release.tag_name.trim_start_matches(['v', 'V']);
    if !force && mod_.manifest.version == Some(version.into()) {
        return Ok((mod_, UpdateResult::AlreadyUpToDate));
    }
    let assets: Vec<_> = release.assets.iter()
        .filter(|asset| asset.name.ends_with(".zip"))
        .collect();
    if assets.len() == 0 {
        return Ok((mod_, UpdateResult::Failure("No valid release assets found".into())));
    }
    if assets.len() > 1 {
        return Ok((mod_, UpdateResult::Failure(format!("Multiple valid assets found: {assets:?}"))));
    }
    let asset = reqwest_client().await
        .get(assets[0].browser_download_url.clone())
        .send().await?
        .error_for_status()?
        .bytes().await?;
    install(&mod_, asset).await?;
    Ok((mod_, UpdateResult::Success {new_version: version.into()}))
}

#[cfg(feature = "nexus")]
async fn nexus(mod_: Mod, update_key: UpdateKey, force: bool) -> Result<(Mod, UpdateResult)> {
    if !force && !check_for_updates(&mod_).await? {
        return Ok((mod_, UpdateResult::AlreadyUpToDate));
    }
    let file_id;
    {
        let document = Html::parse_document(&*reqwest_client().await
            .get(format!("https://nexusmods.com/stardewvalley/mods/{}?tab=files", update_key.id))
            .send().await?
            .error_for_status()?
            .text().await?
        );
        file_id = document.select(&Selector::parse("#file-container-main-files .file-expander-header").unwrap())
            .next()
            .or(document.select(&Selector::parse("#file-container-update-files .file-expander-header").unwrap())
                .next())
            .ok_or(anyhow!("HTML parsing failed: couldn't locate main file element"))?
            .attr("data-id").ok_or(anyhow!("HTML parsing failed: couldn't find data-id attribute"))?
            .to_string();
    }
    let auth = NEXUS_AUTH.get_or_try_init(|| async {
        Result::<_, anyhow::Error>::Ok(
            rookie::load(Some(vec! ["nexusmods.com"])).unwrap().iter()
                .find(|cookie| cookie.name == "sid_develop")
                .ok_or(anyhow!("Could not find Nexus cookie. Please sign in to Nexus in any web browser and try again."))?
                .value.clone()
        )
    }).await?;
    let url = reqwest_client().await
        .post("https://www.nexusmods.com/Core/Libs/Common/Managers/Downloads?GenerateDownloadUrl")
        .form(&json!(
            {
                "fid": file_id,
                "game_id": 1303,
            }
        ))
        .header("Cookie", format!("sid_develop={auth}"))
        .send().await?
        .error_for_status()?
        .json::<serde_json::Value>().await?
        .get("url").ok_or(anyhow!("Invalid response from Nexus: Expected url"))?
        .as_str().ok_or(anyhow!("Invalid response from Nexus: Expected string"))?
        .to_string();
    let file = reqwest_client().await
        .get(url)
        .send().await?
        .error_for_status()?
        .bytes().await?;
    install(&mod_, file).await?;
    Ok((mod_, UpdateResult::Success {new_version: "latest".into()}))  // TODO: Determine actual new version
}

async fn install(mod_: &Mod, archive: Bytes) -> Result<()> {
    let old_dir = mod_.path.join("../.old/").canonicalize()?;
    if !old_dir.is_dir() {
        fs::create_dir(&old_dir).await?;
    }
    let old_subdir = old_dir.join(format!("{} - {}", mod_.path.file_name().unwrap().to_string_lossy(), mod_.manifest.version.clone().unwrap_or("unknown".into())));
    if old_subdir.is_dir() {
        bail!("File exists: {}", old_subdir.display());
    }
    let mut zip = ZipArchive::new(Cursor::new(archive))?;
    if zip.is_empty() {
        bail!("Zip archive is empty");
    }
    let base: HashSet<_> = zip.file_names().filter_map(|name| Some(name.split_once('/')?.0)).collect();
    if base.len() != 1 {
        bail!("Zip archive must contain exactly one item in the root");
    }
    let base = base.iter().next().unwrap();
    let new_dir = mod_.path.with_file_name(base);
    fs::rename(&mod_.path, &old_subdir).await?;
    if new_dir.exists() {
        bail!("File exists: {}", base);
    }
    zip.extract(mod_.path.parent().unwrap())?;
    for file in old_subdir
        .read_dir()?
        .flatten()
        .filter(|file| ["config.json", "config"].contains(&&*file.file_name().to_string_lossy()))
    {
        let new_path = new_dir.join(file.path().strip_prefix(&old_subdir)?);
        if new_path.exists() {
            fs::rename(&new_path, new_path.with_file_name(format!("{}.new", file.file_name().to_string_lossy()))).await?;
        }
        copy(&file.path(), &new_path).await?;
    }
    Ok(())
}

#[async_recursion]
async fn copy(from: &PathBuf, to: &PathBuf) -> Result<()> {
    if from.is_file() {
        fs::copy(from, to).await?;
    } else if from.is_dir() {
        fs::create_dir(&to).await?;
        for file in from.read_dir()?.flatten() {
            copy(&file.path(), &to.join(file.file_name())).await?;
        }
    } else {
        bail!("{} is neither a file nor a directory", from.display());
    }
    Ok(())
}
