use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufReader, Cursor, Read},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("could not read the save file")]
    Serde(#[from] serde_json::Error),
    #[error("an I/O error occured")]
    Io(#[from] std::io::Error),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClipSave {
    pub title: String,
    pub image_path: PathBuf,
    pub music_path: PathBuf,
    #[serde(default)]
    pub offset: Duration,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SaveFile {
    pub clips: Vec<ClipSave>,
    pub timeline: Vec<Option<String>>,
    pub settings: Settings,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settings {
    pub duration: u32,
    pub countdown: Option<PathBuf>,
}

#[inline]
pub fn load<P: AsRef<Path>>(path: P) -> Result<SaveFile, Error> {
    SaveFile::load(path)
}

#[inline]
pub fn store<P: AsRef<Path>>(path: P, save: &SaveFile) -> Result<(), Error> {
    save.store(path)
}

impl SaveFile {
    pub fn data(&self) -> Result<(usize, impl Read), Error> {
        let data = serde_json::to_vec(&self)?;
        Ok((data.len(), Cursor::new(data)))
    }

    pub fn store<P: AsRef<Path>>(&self, path: P) -> Result<(), Error> {
        Ok(serde_json::to_writer(File::create(path)?, self)?)
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        Ok(serde_json::from_reader(BufReader::new(File::open(path)?))?)
    }
}
