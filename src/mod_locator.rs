use std::fs;
use std::path::PathBuf;
use anyhow::anyhow;
use json_comments::StripComments;
use serde::{Deserialize, Deserializer};
use serde::de::Error;

#[derive(Debug)]
pub struct Mod {
    pub path: PathBuf,
    pub manifest: Manifest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Manifest {
    pub name: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub unique_id: Option<String>,
    #[serde(default)]
    pub update_keys: Vec<UpdateKey>,
}

#[derive(Debug)]
pub struct UpdateKey {
    pub source: String,
    pub id: String,
    pub subkey: Option<String>,
}

pub trait UpdateKeys {
    fn preferred(&self) -> Option<&UpdateKey>;
}

impl UpdateKeys for [UpdateKey] {
    fn preferred(&self) -> Option<&UpdateKey> {
        self.iter().find(|key| key.source.to_lowercase() == "github")
            .or_else(|| self.iter().find(|key| key.source.to_lowercase() == "nexus"))
    }
}

impl<'de> Deserialize<'de> for UpdateKey {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error> where D: Deserializer<'de> {
        let value = String::deserialize(deserializer)?;
        let (source, key) = value.split_once(':').ok_or(Error::custom("UpdateKey must contain ':'"))?;
        if let Some((id, subkey)) = key.split_once('@') {
            Ok(Self {
                source: source.into(),
                id: id.into(),
                subkey: Some(format!("@{subkey}")),
            })
        } else {
            Ok(Self {
                source: source.into(),
                id: key.into(),
                subkey: None,
            })
        }
    }
}

pub fn locate_mods(dir: &PathBuf) -> anyhow::Result<Vec<Mod>> {
    let mut mods = Vec::with_capacity(dir.read_dir()?.count());
    for subdir in dir.read_dir()?.flatten() {
        if !subdir.file_type()?.is_dir() || subdir.file_name().to_str().ok_or(anyhow!("Invalid directory name"))?.starts_with(".") {
            continue
        }
        let manifest = subdir.path().join("manifest.json");
        if manifest.is_file() {
            mods.push(Mod {
                path: subdir.path(),
                manifest: serde_json::from_reader(StripComments::new(fs::read_to_string(manifest)?.trim_start_matches('\u{feff}').as_bytes()))?,
            });
        } else {
            mods.extend(locate_mods(&subdir.path())?);
        }
    }
    Ok(mods)
}
