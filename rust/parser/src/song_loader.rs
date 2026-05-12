use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::index::song_root_folder;
use crate::models::SongData;

pub fn load_song(folder_name: &str) -> Result<SongData> {
    let path = song_root_folder().join(folder_name).join("data.json");
    load_song_from_path(&path)
}

pub fn load_song_from_path(path: &Path) -> Result<SongData> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed reading song data '{}'", path.display()))?;
    let mut song = serde_json::from_str::<SongData>(&content)
        .with_context(|| format!("failed parsing song data '{}'", path.display()))?;
    if let Some(lyrics) = &mut song.lyrics {
        for line in &mut lyrics.lines {
            line.refresh_timing();
        }
    }
    Ok(song)
}
