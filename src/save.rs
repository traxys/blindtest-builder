use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Serialize, Deserialize)]
pub struct ClipSave {
    pub title: String,
    pub image_path: PathBuf,
    pub music_path: PathBuf,
    #[serde(default)]
    pub offset: Duration,
}

#[derive(Serialize, Deserialize)]
pub struct SaveFile {
    pub clips: Vec<ClipSave>,
    pub timeline: Vec<Option<String>>,
    pub settings: Settings,
}

#[derive(Serialize, Deserialize)]
pub struct Settings {
    pub duration: u32,
    pub countdown: Option<PathBuf>,
}

pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<SaveFile> {
    Ok(serde_json::from_reader(BufReader::new(File::open(path)?))?)
}

pub fn store<P: AsRef<Path>>(path: P, save: &SaveFile) -> anyhow::Result<()> {
    Ok(serde_json::to_writer(File::create(path)?, save)?)
}
