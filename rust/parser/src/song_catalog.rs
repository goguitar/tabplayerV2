use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;

use crate::index::{
    load_album_art_dds_from_psarc, load_audio_wav_bytes_from_psarc, load_song_data_from_psarc,
    parse_song_file_summary_from_psarc,
};
use crate::models::{SongData, SongFile};

pub const DEFAULT_DLC_DIR: &str = "/home/csantz/Music/DLC";

#[derive(Clone)]
struct CatalogEntry {
    song_id: String,
    psarc_path: PathBuf,
    song_file: SongFile,
}

#[derive(Default)]
struct CatalogState {
    entries: Vec<CatalogEntry>,
    scan_root: PathBuf,
}

static SONG_CATALOG: Lazy<Mutex<CatalogState>> = Lazy::new(|| Mutex::new(CatalogState::default()));

pub fn rescan_default_dlc() -> Result<usize> {
    rescan_dlc_dir(Path::new(DEFAULT_DLC_DIR))
}

pub fn rescan_dlc_dir(root: &Path) -> Result<usize> {
    let paths = collect_psarc_files_recursive(root);
    let mut entries = Vec::new();
    let mut used_ids = HashSet::<String>::new();

    for path in paths {
        let Ok(mut song_file) = parse_song_file_summary_from_psarc(&path) else {
            continue;
        };

        if song_file.album == "Unknown Album" || song_file.year.is_none() || song_file.length <= 0.0 {
            if let Ok(song_data) = load_song_data_from_psarc(&path) {
                if song_file.song_name.trim().is_empty() || song_file.song_name.eq_ignore_ascii_case("unknown song") {
                    song_file.song_name = song_data.metadata.name;
                }
                if song_file.artist.trim().is_empty() || song_file.artist.eq_ignore_ascii_case("unknown artist") {
                    song_file.artist = song_data.metadata.artist;
                }
                if song_file.album == "Unknown Album" && !song_data.metadata.album.trim().is_empty() {
                    song_file.album = song_data.metadata.album;
                }
                if song_file.year.is_none() {
                    song_file.year = song_data.metadata.year;
                }
                if song_file.length <= 0.0 {
                    song_file.length = song_data.metadata.song_length;
                }
            }
        }

        let base_id = if song_file.folder_name.is_empty() {
            "song".to_string()
        } else {
            song_file.folder_name.clone()
        };

        let mut song_id = base_id.clone();
        let mut suffix = 2_u32;
        while !used_ids.insert(song_id.clone()) {
            song_id = format!("{base_id}_{suffix}");
            suffix += 1;
        }

        song_file.folder_name = song_id.clone();
        entries.push(CatalogEntry {
            song_id,
            psarc_path: path,
            song_file,
        });
    }

    entries.sort_by(|a, b| a.song_file.song_name.cmp(&b.song_file.song_name));

    let mut lock = SONG_CATALOG
        .lock()
        .map_err(|_| anyhow!("song catalog lock poisoned"))?;
    lock.entries = entries;
    lock.scan_root = root.to_path_buf();
    Ok(lock.entries.len())
}

pub fn ensure_default_loaded() -> Result<()> {
    let needs_scan = SONG_CATALOG
        .lock()
        .map_err(|_| anyhow!("song catalog lock poisoned"))?
        .entries
        .is_empty();
    if needs_scan {
        let _ = rescan_default_dlc()?;
    }
    Ok(())
}

pub fn list_song_files() -> Vec<SongFile> {
    if ensure_default_loaded().is_err() {
        return Vec::new();
    }
    SONG_CATALOG
        .lock()
        .ok()
        .map(|x| x.entries.iter().map(|e| e.song_file.clone()).collect())
        .unwrap_or_default()
}

pub fn get_song_file(song_id: &str) -> Option<SongFile> {
    ensure_default_loaded().ok()?;
    SONG_CATALOG
        .lock()
        .ok()?
        .entries
        .iter()
        .find(|x| x.song_id == song_id)
        .map(|x| x.song_file.clone())
}

pub fn load_song_data(song_id: &str) -> Result<SongData> {
    ensure_default_loaded()?;
    let path = SONG_CATALOG
        .lock()
        .map_err(|_| anyhow!("song catalog lock poisoned"))?
        .entries
        .iter()
        .find(|x| x.song_id == song_id)
        .map(|x| x.psarc_path.clone())
        .ok_or_else(|| anyhow!("song id not found: {song_id}"))?;
    load_song_data_from_psarc(&path)
}

pub fn load_song_audio_wav(song_id: &str) -> Result<Vec<u8>> {
    ensure_default_loaded()?;
    let path = SONG_CATALOG
        .lock()
        .map_err(|_| anyhow!("song catalog lock poisoned"))?
        .entries
        .iter()
        .find(|x| x.song_id == song_id)
        .map(|x| x.psarc_path.clone())
        .ok_or_else(|| anyhow!("song id not found: {song_id}"))?;
    load_audio_wav_bytes_from_psarc(&path)?.ok_or_else(|| anyhow!("song audio wem not found"))
}

pub fn load_song_album_art(song_id: &str) -> Result<Option<Vec<u8>>> {
    ensure_default_loaded()?;
    let path = SONG_CATALOG
        .lock()
        .map_err(|_| anyhow!("song catalog lock poisoned"))?
        .entries
        .iter()
        .find(|x| x.song_id == song_id)
        .map(|x| x.psarc_path.clone())
        .ok_or_else(|| anyhow!("song id not found: {song_id}"))?;
    load_album_art_dds_from_psarc(&path)
}

fn collect_psarc_files_recursive(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return files;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_psarc_files_recursive(&path));
            continue;
        }
        if path.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("psarc")) {
            files.push(path);
        }
    }

    files
}
