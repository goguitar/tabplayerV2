use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::process;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rocksmith2014_psarc::Psarc;
use rocksmith2014_sng::{BendValue, Note as SngNote, NoteMask as SngNoteMask, Platform as SngPlatform, Sng};
use rocksmith2014_xml::{ChordTemplate, InstrumentalArrangement};
use serde_json::Value;
use tabplayer_ffi::native_audio::decode_wem_to_wav;

use crate::models::{
    InstrumentConfig, LyricLine, LyricWord, NoteBlock, NoteBlockFlags, NoteType, SingleBend, SingleNote,
    SingleSlide, SongData, SongFile, SongFileInstrument, SongFileList, SongLyrics, SongMetadata, SongInstrument,
};

#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub completed: usize,
    pub failed: usize,
    pub entries: Vec<ImportEntry>,
}

#[derive(Debug, Clone)]
pub struct ImportEntry {
    pub source_path: PathBuf,
    pub folder_name: String,
}

pub fn song_root_folder() -> PathBuf {
    if let Ok(local) = env::var("LOCALAPPDATA") {
        return Path::new(&local).join("murph9.TabPlayer");
    }
    if let Ok(xdg) = env::var("XDG_DATA_HOME") {
        return Path::new(&xdg).join("murph9.TabPlayer");
    }
    if let Ok(home) = env::var("HOME") {
        return Path::new(&home).join(".local/share/murph9.TabPlayer");
    }
    PathBuf::from(".")
}

pub fn play_data_file() -> PathBuf {
    song_root_folder().join("playData.json")
}

pub fn read_song_file_list() -> SongFileList {
    let path = play_data_file();
    let Ok(content) = fs::read_to_string(path) else {
        return SongFileList::default();
    };
    serde_json::from_str::<SongFileList>(&content).unwrap_or_default()
}

pub fn write_song_file_list(list: &SongFileList) -> Result<()> {
    let path = play_data_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create song root '{}'", parent.display()))?;
    }

    let content = serde_json::to_string_pretty(list)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn parse_song_file_summary_from_psarc(path: &Path) -> Result<SongFile> {
    let file_stem = path
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or("unknown_song");
    let folder_name = sanitize_folder_name(file_stem);

    let mut psarc = Psarc::open(path).with_context(|| format!("failed opening psarc '{}'", path.display()))?;
    let manifest = psarc.manifest().to_vec();

    let arrangement_paths = manifest
        .iter()
        .filter(|x| {
            x.starts_with("songs/arr/")
                && x.ends_with(".xml")
                && !x.contains("vocals")
                && !x.contains("showlights")
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut parsed_arrangements = Vec::new();
    for arrangement_path in arrangement_paths {
        let data = psarc.inflate_file(&arrangement_path)?;
        let xml = String::from_utf8(data)
            .with_context(|| format!("invalid utf8 xml '{}'", arrangement_path))?;
        let arrangement = InstrumentalArrangement::from_xml(&xml)
            .map_err(|e| anyhow::anyhow!("failed parsing arrangement '{}': {e}", arrangement_path))?;
        parsed_arrangements.push((arrangement_path, arrangement));
    }

    let mut metadata = metadata_from_arrangements(file_stem, &parsed_arrangements);
    if let Some(hsan_metadata) = load_hsan_song_metadata(&mut psarc, &manifest, file_stem) {
        apply_song_metadata(&mut metadata, hsan_metadata, true, file_stem);
    } else if let Some(manifest_metadata) = load_manifest_song_metadata(&mut psarc, &manifest, file_stem)
    {
        apply_song_metadata(&mut metadata, manifest_metadata, false, file_stem);
    }

    let mut instruments = Vec::<SongFileInstrument>::new();
    if parsed_arrangements.is_empty() {
        let mut names = manifest
            .iter()
            .filter(|x| x.to_ascii_lowercase().ends_with(".sng"))
            .filter_map(|x| instrument_name_from_sng_path(x))
            .filter(|name| name != "vocals" && name != "showlights")
            .collect::<Vec<_>>();
        names.sort_by_key(|x| instrument_order_key(x));
        names.dedup();
        for (i, name) in names.into_iter().enumerate() {
            instruments.push(SongFileInstrument {
                name,
                is_main: i == 0,
                tuning: vec![0; 6],
                note_count: 0,
                capo_fret: 0.0,
            });
        }
    } else {
        for (arrangement_path, arrangement) in parsed_arrangements {
            let instrument_name = instrument_name_from_path(&arrangement_path);
            let level = arrangement
                .levels
                .iter()
                .max_by_key(|x| x.difficulty)
                .or_else(|| arrangement.levels.first());
            let note_count = level
                .map(|x| x.notes.len() + x.chords.len())
                .unwrap_or(0);
            instruments.push(SongFileInstrument {
                name: instrument_name,
                is_main: false,
                tuning: arrangement.meta.tuning.strings.iter().copied().collect(),
                note_count,
                capo_fret: arrangement.meta.capo as f32,
            });
        }
        instruments.sort_by_key(|x| instrument_order_key(&x.name));
        for (i, inst) in instruments.iter_mut().enumerate() {
            inst.is_main = i == 0;
        }
    }

    if metadata.song_length <= 0.0 {
        metadata.song_length = 0.0;
    }

    let has_vocals = manifest
        .iter()
        .any(|x| x.to_ascii_lowercase().ends_with("_vocals.sng") || x.to_ascii_lowercase().contains("vocals"));

    Ok(SongFile {
        folder_name,
        song_name: metadata.name,
        artist: metadata.artist,
        album: metadata.album,
        year: metadata.year,
        length: metadata.song_length,
        instruments,
        lyrics: if has_vocals {
            Some(crate::models::SongFileLyrics { word_count: 1 })
        } else {
            None
        },
    })
}

pub fn parse_song_file_from_psarc(path: &Path) -> Result<SongFile> {
    let file_stem = path
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or("unknown_song");
    let folder_name = sanitize_folder_name(file_stem);
    let (_, song_file) = parse_song_models_from_psarc(path, folder_name)?;
    Ok(song_file)
}

pub fn load_song_data_from_psarc(path: &Path) -> Result<SongData> {
    let file_stem = path
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or("unknown_song");
    let folder_name = sanitize_folder_name(file_stem);
    let (song_data, _) = parse_song_models_from_psarc(path, folder_name)?;
    Ok(song_data)
}

pub fn load_album_art_dds_from_psarc(path: &Path) -> Result<Option<Vec<u8>>> {
    let mut psarc = Psarc::open(path).with_context(|| format!("failed opening psarc '{}'", path.display()))?;
    let manifest = psarc.manifest().to_vec();
    let Some(art_path) = pick_album_art_path(&manifest) else {
        return Ok(None);
    };
    let bytes = psarc.inflate_file(&art_path)?;
    Ok(Some(bytes))
}

pub fn load_audio_wav_bytes_from_psarc(path: &Path) -> Result<Option<Vec<u8>>> {
    let mut psarc = Psarc::open(path).with_context(|| format!("failed opening psarc '{}'", path.display()))?;
    let manifest = psarc.manifest().to_vec();
    let Some(wem_path) = pick_audio_wem_path(&mut psarc, &manifest) else {
        return Ok(None);
    };
    let wem_bytes = psarc.inflate_file(&wem_path)?;
    let wav_bytes = decode_wem_bytes_to_wav_bytes(&wem_bytes)?;
    Ok(Some(wav_bytes))
}

pub fn import_psarc_files(paths: &[PathBuf]) -> Result<ImportReport> {
    let mut report = ImportReport::default();
    let root = song_root_folder();
    fs::create_dir_all(&root)?;

    let mut list = read_song_file_list();

    for path in paths {
        if path.extension().and_then(|x| x.to_str()) != Some("psarc") {
            report.failed += 1;
            continue;
        }

        match import_single_psarc(path) {
            Ok((song_file, entry)) => {
                upsert_song_file(&mut list, song_file);
                report.completed += 1;
                report.entries.push(entry);
            }
            Err(_) => {
                report.failed += 1;
            }
        }
    }

    list.data.sort_by(|a, b| a.song_name.cmp(&b.song_name));
    write_song_file_list(&list)?;
    Ok(report)
}

fn import_single_psarc(path: &Path) -> Result<(SongFile, ImportEntry)> {
    let file_stem = path
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or("unknown_song");
    let folder_name = sanitize_folder_name(file_stem);

    let (song_data, song_file) = parse_song_models_from_psarc(path, folder_name.clone())?;

    let song_dir = song_root_folder().join(&folder_name);
    fs::create_dir_all(&song_dir)?;

    let copied_psarc = song_dir.join(
        path.file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("song.psarc"),
    );
    fs::copy(path, copied_psarc).with_context(|| format!("failed copying '{}'", path.display()))?;

    let data_path = song_dir.join("data.json");
    fs::write(&data_path, serde_json::to_vec_pretty(&song_data)?)?;

    let mut psarc = Psarc::open(path).with_context(|| format!("failed opening psarc '{}'", path.display()))?;
    let manifest = psarc.manifest().to_vec();

    if let Some(art_path) = pick_album_art_path(&manifest) {
        if let Ok(bytes) = psarc.inflate_file(&art_path) {
            let art_file = song_dir.join("album.dds");
            let _ = fs::write(art_file, bytes);
        }
    }

    if let Some(wem_path) = pick_audio_wem_path(&mut psarc, &manifest) {
        if let Ok(bytes) = psarc.inflate_file(&wem_path) {
            let wem_file = song_dir.join("song.wem");
            let _ = fs::write(&wem_file, bytes);
            let wav_file = song_dir.join("song.wav");
            match decode_wem_to_wav(&wem_file) {
                Ok(wav_bytes) => {
                    if let Err(err) = fs::write(&wav_file, wav_bytes) {
                        eprintln!("[audio] failed writing wav for '{}': {err}", wem_file.display());
                    }
                }
                Err(err) => {
                    eprintln!("[audio] failed decoding wem '{}': {err}", wem_file.display());
                }
            }
        }
    }

    Ok((
        song_file,
        ImportEntry {
            source_path: path.to_path_buf(),
            folder_name,
        },
    ))
}

fn parse_song_models_from_psarc(path: &Path, folder_name: String) -> Result<(SongData, SongFile)> {
    let file_stem = path
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or("unknown_song");

    let mut psarc = Psarc::open(path).with_context(|| format!("failed opening psarc '{}'", path.display()))?;
    let manifest = psarc.manifest().to_vec();

    let arrangement_paths = manifest
        .iter()
        .filter(|x| {
            x.starts_with("songs/arr/")
                && x.ends_with(".xml")
                && !x.contains("vocals")
                && !x.contains("showlights")
        })
        .cloned()
        .collect::<Vec<_>>();

    let mut parsed_arrangements = Vec::new();
    for arrangement_path in arrangement_paths {
        let data = psarc.inflate_file(&arrangement_path)?;
        let xml = String::from_utf8(data)
            .with_context(|| format!("invalid utf8 xml '{}'", arrangement_path))?;
        let arrangement = InstrumentalArrangement::from_xml(&xml)
            .map_err(|e| anyhow::anyhow!("failed parsing arrangement '{}': {e}", arrangement_path))?;
        parsed_arrangements.push((arrangement_path, arrangement));
    }

    let mut sng_by_instrument: HashMap<String, Sng> = HashMap::new();
    let mut vocals_sng: Option<Sng> = None;
    for entry in &manifest {
        if !entry.to_ascii_lowercase().ends_with(".sng") {
            continue;
        }
        let Ok(bytes) = psarc.inflate_file(entry) else {
            continue;
        };
        let Some(sng) = read_sng_any_platform(&bytes) else {
            continue;
        };
        if entry.to_ascii_lowercase().contains("vocals") {
            vocals_sng = Some(sng);
            continue;
        }
        if let Some(name) = instrument_name_from_sng_path(entry) {
            sng_by_instrument.entry(name).or_insert(sng);
        }
    }

    let mut metadata = metadata_from_arrangements(file_stem, &parsed_arrangements);
    if let Some(hsan_metadata) = load_hsan_song_metadata(&mut psarc, &manifest, file_stem) {
        apply_song_metadata(&mut metadata, hsan_metadata, true, file_stem);
    } else if let Some(manifest_metadata) = load_manifest_song_metadata(&mut psarc, &manifest, file_stem)
    {
        apply_song_metadata(&mut metadata, manifest_metadata, false, file_stem);
    }

    let mut instruments = Vec::new();
    if parsed_arrangements.is_empty() {
        for (instrument_name, sng) in &sng_by_instrument {
            let instrument = convert_sng_only_instrument(instrument_name, sng);
            instruments.push(instrument);
        }
    } else {
        for (arrangement_path, arrangement) in parsed_arrangements {
            let instrument_name = instrument_name_from_path(&arrangement_path);
            let instrument = convert_instrument(&instrument_name, &arrangement, sng_by_instrument.get(&instrument_name));
            instruments.push(instrument);
        }
    }
    instruments.sort_by_key(|x| instrument_order_key(&x.name));

    if instruments.is_empty() {
        anyhow::bail!("no playable instruments in '{}'", path.display());
    }

    if metadata.song_length <= 0.0 {
        metadata.song_length = instruments
            .iter()
            .filter_map(|inst| inst.notes.last().map(|n| n.time))
            .fold(0.0_f64, f64::max);
    }

    let lyrics = vocals_sng.as_ref().and_then(parse_vocals_from_sng);

    let song_data = SongData {
        metadata: metadata.clone(),
        instruments: instruments.clone(),
        lyrics,
    };

    let song_file = SongFile {
        folder_name,
        song_name: metadata.name,
        artist: metadata.artist,
        album: metadata.album,
        year: metadata.year,
        length: metadata.song_length,
        instruments: instruments
            .iter()
            .enumerate()
            .map(|(i, instrument)| SongFileInstrument {
                name: instrument.name.clone(),
                is_main: i == 0,
                tuning: instrument.config.tuning.clone(),
                note_count: instrument.notes.len(),
                capo_fret: instrument.config.capo_fret,
            })
            .collect(),
        lyrics: song_data.lyrics.as_ref().map(|lyrics| crate::models::SongFileLyrics {
            word_count: lyrics.lines.iter().map(|x| x.words.len()).sum(),
        }),
    };

    Ok((song_data, song_file))
}

fn metadata_from_file_stem(file_stem: &str) -> SongMetadata {
    let normalized = file_stem.replace('_', " ").replace('-', " ");
    let (artist, name) = if let Some((artist, title)) = file_stem.split_once('_') {
        (
            artist.replace('-', " ").trim().to_string(),
            title.replace('-', " ").replace('_', " ").trim().to_string(),
        )
    } else {
        ("Unknown Artist".to_string(), normalized.trim().to_string())
    };

    SongMetadata {
        name: if name.is_empty() {
            "Unknown Song".to_string()
        } else {
            name
        },
        artist: if artist.is_empty() {
            "Unknown Artist".to_string()
        } else {
            artist
        },
        album: "Unknown Album".to_string(),
        year: None,
        song_length: 0.0,
    }
}

fn metadata_from_arrangements(
    file_stem: &str,
    parsed_arrangements: &[(String, InstrumentalArrangement)],
) -> SongMetadata {
    let mut metadata = metadata_from_file_stem(file_stem);
    let default_name = file_stem.replace('_', " ");

    for (_, arrangement) in parsed_arrangements {
        if (metadata.name.is_empty() || metadata.name == default_name || metadata.name == "Unknown Song")
            && !arrangement.meta.song_name.is_empty()
        {
            metadata.name = arrangement.meta.song_name.clone();
        }
        if (metadata.artist.is_empty() || metadata.artist == "Unknown Artist")
            && !arrangement.meta.artist_name.is_empty()
        {
            metadata.artist = arrangement.meta.artist_name.clone();
        }
        if (metadata.album.is_empty() || metadata.album == "Unknown Album")
            && !arrangement.meta.album_name.is_empty()
        {
            metadata.album = arrangement.meta.album_name.clone();
        }
        if metadata.year.is_none() && arrangement.meta.album_year > 0 {
            metadata.year = Some(arrangement.meta.album_year);
        }
        if metadata.song_length <= 0.0 && arrangement.meta.song_length > 0 {
            metadata.song_length = arrangement.meta.song_length as f64 / 1000.0;
        }
    }

    if metadata.song_length <= 0.0 {
        metadata.song_length = parsed_arrangements
            .iter()
            .map(|(_, arrangement)| arrangement_max_time_seconds(arrangement))
            .fold(0.0_f64, f64::max);
    }

    metadata
}

fn arrangement_max_time_seconds(arrangement: &InstrumentalArrangement) -> f64 {
    arrangement
        .levels
        .iter()
        .map(|level| {
            let note_max = level
                .notes
                .iter()
                .map(|note| note.time as f64)
                .fold(0.0_f64, f64::max);
            let chord_max = level
                .chords
                .iter()
                .map(|chord| chord.time as f64)
                .fold(0.0_f64, f64::max);
            note_max.max(chord_max)
        })
        .fold(0.0_f64, f64::max)
        / 1000.0
}

fn apply_song_metadata(target: &mut SongMetadata, source: SongMetadata, strict_override: bool, file_stem: &str) {
    let default_name = file_stem.replace('_', " ");

    if !source.name.is_empty()
        && source.name != "Unknown Song"
        && (strict_override
            || target.name.is_empty()
            || target.name == default_name
            || target.name == "Unknown Song")
    {
        target.name = source.name;
    }
    if !source.artist.is_empty()
        && source.artist != "Unknown Artist"
        && (strict_override || target.artist.is_empty() || target.artist == "Unknown Artist")
    {
        target.artist = source.artist;
    }
    if !source.album.is_empty()
        && source.album != "Unknown Album"
        && (strict_override || target.album.is_empty() || target.album == "Unknown Album")
    {
        target.album = source.album;
    }
    if source.year.is_some() && (strict_override || target.year.is_none()) {
        target.year = source.year;
    }
    if source.song_length > 0.0 && (strict_override || target.song_length <= 0.0) {
        target.song_length = source.song_length;
    }
}

fn load_hsan_song_metadata<R: std::io::Read + std::io::Seek>(
    psarc: &mut Psarc<R>,
    manifest: &[String],
    file_stem: &str,
) -> Option<SongMetadata> {
    load_metadata_from_json_entries(psarc, manifest, file_stem, |path| {
        path.to_ascii_lowercase().ends_with(".hsan")
    })
}

fn load_manifest_song_metadata<R: std::io::Read + std::io::Seek>(
    psarc: &mut Psarc<R>,
    manifest: &[String],
    file_stem: &str,
) -> Option<SongMetadata> {
    load_metadata_from_json_entries(psarc, manifest, file_stem, |path| {
        let lower = path.to_ascii_lowercase();
        lower.ends_with(".json") && lower.contains("manif")
    })
}

fn load_metadata_from_json_entries<R: std::io::Read + std::io::Seek, F: Fn(&str) -> bool>(
    psarc: &mut Psarc<R>,
    manifest: &[String],
    file_stem: &str,
    path_filter: F,
) -> Option<SongMetadata> {
    let default = metadata_from_file_stem(file_stem);
    let default_name = default.name.clone();
    let default_artist = default.artist.clone();

    let mut best: Option<(SongMetadata, i32)> = None;

    for entry in manifest {
        if !path_filter(entry) {
            continue;
        }
        let Ok(bytes) = psarc.inflate_file(entry) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };

        let mut candidates = Vec::new();
        collect_attribute_objects(&value, &mut candidates);
        for obj in candidates {
            let mut candidate = SongMetadata {
                name: json_string(obj, &["songName", "SongName"]).unwrap_or_else(|| default_name.clone()),
                artist: json_string(obj, &["artistName", "ArtistName"]).unwrap_or_else(|| default_artist.clone()),
                album: json_string(obj, &["albumName", "AlbumName"]).unwrap_or_else(|| "Unknown Album".to_string()),
                year: json_i32(obj, &["songYear", "SongYear", "albumYear", "AlbumYear"]),
                song_length: json_f64(obj, &["songLength", "SongLength"])
                    .map(|x| if x > 1000.0 { x / 1000.0 } else { x })
                    .unwrap_or(0.0),
            };
            if candidate.year.is_some_and(|x| x <= 0) {
                candidate.year = None;
            }
            if candidate.song_length < 0.0 {
                candidate.song_length = 0.0;
            }

            let score = metadata_score(&candidate, &default_name, &default_artist);
            if best.as_ref().is_none_or(|(_, best_score)| score > *best_score) {
                best = Some((candidate, score));
            }
        }
    }

    best.map(|(metadata, _)| metadata)
}

fn metadata_score(metadata: &SongMetadata, default_name: &str, default_artist: &str) -> i32 {
    let mut score = 0;
    if !metadata.name.is_empty() && metadata.name != "Unknown Song" && metadata.name != default_name {
        score += 2;
    }
    if !metadata.artist.is_empty() && metadata.artist != "Unknown Artist" && metadata.artist != default_artist {
        score += 2;
    }
    if !metadata.album.is_empty() && metadata.album != "Unknown Album" {
        score += 3;
    }
    if metadata.year.is_some_and(|x| x > 0) {
        score += 2;
    }
    if metadata.song_length > 0.0 {
        score += 2;
    }
    score
}

fn collect_attribute_objects<'a>(value: &'a Value, out: &mut Vec<&'a serde_json::Map<String, Value>>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::Object(attrs)) = map.get("Attributes") {
                out.push(attrs);
            }
            if let Some(Value::Object(attrs)) = map.get("attributes") {
                out.push(attrs);
            }
            if map.contains_key("songName")
                || map.contains_key("SongName")
                || map.contains_key("albumName")
                || map.contains_key("AlbumName")
                || map.contains_key("songLength")
                || map.contains_key("SongLength")
            {
                out.push(map);
            }
            for child in map.values() {
                collect_attribute_objects(child, out);
            }
        }
        Value::Array(arr) => {
            for child in arr {
                collect_attribute_objects(child, out);
            }
        }
        _ => {}
    }
}

fn json_string(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = map.get(*key) {
            if let Some(s) = value.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn json_i32(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<i32> {
    for key in keys {
        if let Some(value) = map.get(*key) {
            if let Some(n) = value.as_i64() {
                return Some(n as i32);
            }
            if let Some(s) = value.as_str() {
                if let Ok(n) = s.parse::<i32>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn json_f64(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(value) = map.get(*key) {
            if let Some(n) = value.as_f64() {
                return Some(n);
            }
            if let Some(n) = value.as_i64() {
                return Some(n as f64);
            }
            if let Some(s) = value.as_str() {
                if let Ok(n) = s.parse::<f64>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

fn convert_sng_only_instrument(name: &str, sng: &Sng) -> SongInstrument {
    let notes = build_note_blocks_from_sng(sng);
    let string_count = notes
        .iter()
        .flat_map(|x| x.notes.iter().map(|n| n.string_num.max(0) as usize + 1))
        .max()
        .unwrap_or(6)
        .max(4)
        .min(7);

    SongInstrument {
        name: name.to_string(),
        config: InstrumentConfig {
            note_speed: 40.0,
            tuning: vec![0_i16; string_count],
            capo_fret: 0.0,
        },
        notes,
    }
}

fn decode_wem_bytes_to_wav_bytes(wem_bytes: &[u8]) -> Result<Vec<u8>> {
    let temp_name = format!(
        "tabplayer_{}_{}_{}.wem",
        process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        wem_bytes.len()
    );
    let temp_path = env::temp_dir().join(temp_name);
    fs::write(&temp_path, wem_bytes)
        .with_context(|| format!("failed writing temp wem '{}'", temp_path.display()))?;

    let decode_result = decode_wem_to_wav(&temp_path);
    let _ = fs::remove_file(&temp_path);
    decode_result
}

fn convert_instrument(name: &str, arrangement: &InstrumentalArrangement, sng: Option<&Sng>) -> SongInstrument {
    let notes = if let Some(sng) = sng {
        build_note_blocks_from_sng(sng)
    } else {
        let level = arrangement
            .levels
            .iter()
            .max_by_key(|x| x.difficulty)
            .or_else(|| arrangement.levels.first());

        if let Some(level) = level {
            build_note_blocks(arrangement, level)
        } else {
            Vec::new()
        }
    };

    SongInstrument {
        name: name.to_string(),
        config: InstrumentConfig {
            note_speed: 40.0,
            tuning: arrangement.meta.tuning.strings.iter().copied().collect(),
            capo_fret: arrangement.meta.capo as f32,
        },
        notes,
    }
}

fn build_note_blocks(arrangement: &InstrumentalArrangement, level: &rocksmith2014_xml::Level) -> Vec<NoteBlock> {
    let mut out = Vec::new();

    for note in &level.notes {
        let (start, len) = anchor_for_time(level, note.time);
        out.push(NoteBlock {
            label: None,
            time: note.time as f64 / 1000.0,
            fret_window_start: start,
            fret_window_length: len,
            chord_flags: Vec::new(),
            notes: vec![SingleNote {
                string_num: note.string as i32,
                fret_num: note.fret as i32,
                length: note.sustain as f64 / 1000.0,
                note_type: Vec::new(),
                bends: Vec::new(),
                slide: None,
            }],
        });
    }

    for chord in &level.chords {
        let (start, len) = anchor_for_time(level, chord.time);
        let chord_notes = if !chord.chord_notes.is_empty() {
            chord
                .chord_notes
                .iter()
                .map(|n| SingleNote {
                    string_num: n.string as i32,
                    fret_num: n.fret as i32,
                    length: n.sustain as f64 / 1000.0,
                    note_type: vec![NoteType::CHORD],
                    bends: Vec::new(),
                    slide: None,
                })
                .collect::<Vec<_>>()
        } else {
            chord_notes_from_template(arrangement, chord.chord_id)
        };

        if !chord_notes.is_empty() {
            out.push(NoteBlock {
                label: None,
                time: chord.time as f64 / 1000.0,
                fret_window_start: start,
                fret_window_length: len,
                chord_flags: Vec::new(),
                notes: chord_notes,
            });
        }
    }

    out.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn build_note_blocks_from_sng(sng: &Sng) -> Vec<NoteBlock> {
    let mut note_blocks = Vec::new();
    for note in select_sng_notes(sng) {
        if note.mask.contains(SngNoteMask::CHORD) {
            note_blocks.push(from_chord(sng, &note));
        } else {
            note_blocks.push(from_single_note(&note));
        }
    }

    let mut grouped = BTreeMap::<i64, Vec<NoteBlock>>::new();
    for note in note_blocks {
        let key = (note.time * 1000.0).round() as i64;
        grouped.entry(key).or_default().push(note);
    }

    let mut out = Vec::new();
    for group in grouped.into_values() {
        if group.len() > 1 && group.iter().all(|x| x.notes.len() == 1) {
            let first_time = group[0].time;
            let first_fret_window_start = group[0].fret_window_start;
            let first_fret_window_length = group[0].fret_window_length;
            let mut notes = Vec::new();
            for n in group {
                notes.extend(n.notes);
            }
            out.push(NoteBlock {
                label: None,
                time: first_time,
                fret_window_start: first_fret_window_start,
                fret_window_length: first_fret_window_length,
                chord_flags: Vec::new(),
                notes,
            });
        } else {
            out.extend(group);
        }
    }

    out.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn select_sng_notes(sng: &Sng) -> Vec<SngNote> {
    let mut levels = sng.levels.iter().collect::<Vec<_>>();
    levels.sort_by_key(|x| -x.difficulty);

    if sng.phrase_iterations.is_empty() {
        return levels.first().map(|x| x.notes.clone()).unwrap_or_default();
    }

    let mut all = Vec::new();
    for (phrase_idx, _) in sng.phrase_iterations.iter().enumerate() {
        for level in &levels {
            let mut phrase_notes = level
                .notes
                .iter()
                .filter(|n| n.phrase_iteration_id == phrase_idx as i32)
                .cloned()
                .collect::<Vec<_>>();
            if !phrase_notes.is_empty() {
                all.append(&mut phrase_notes);
                break;
            }
        }
    }

    all.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));
    all
}

fn from_single_note(note: &SngNote) -> NoteBlock {
    let mut flags = convert_to_model(note.mask);
    let mut slide = None;
    if note.slide_to >= 0 {
        slide = Some(SingleSlide {
            to_fret: note.slide_to as i32,
            slide_unpitched: false,
        });
    }
    if note.slide_unpitch_to >= 0 {
        slide = Some(SingleSlide {
            to_fret: note.slide_unpitch_to as i32,
            slide_unpitched: true,
        });
    }

    let bends = from_bend_values(&note.bend_data, note.time, note.sustain);
    if bends.is_empty() {
        flags.retain(|x| *x != NoteType::BEND);
    }

    if note.sustain <= 0.0 {
        slide = None;
    }

    NoteBlock {
        label: None,
        time: note.time as f64,
        fret_window_start: (note.anchor_fret as i32).max(1),
        fret_window_length: (note.anchor_width as i32).max(1),
        chord_flags: Vec::new(),
        notes: vec![SingleNote {
            string_num: note.string_index as i32,
            fret_num: note.fret as i32,
            length: note.sustain as f64,
            note_type: flags,
            bends,
            slide,
        }],
    }
}

fn from_chord(sng: &Sng, note: &SngNote) -> NoteBlock {
    let base_flags = convert_to_model(note.mask);
    let chord = usize::try_from(note.chord_id).ok().and_then(|idx| sng.chords.get(idx));
    let chord_notes = usize::try_from(note.chord_notes_id)
        .ok()
        .and_then(|idx| sng.chord_notes.get(idx));

    let mut list = Vec::new();
    if let Some(chord) = chord {
        for i in 0..6 {
            if chord.frets[i] < 0 {
                continue;
            }

            let mut flags = base_flags.clone();
            let mut slide = None;
            let mut bends = Vec::new();

            if let Some(chord_note) = chord_notes {
                flags.extend(convert_to_model(SngNoteMask::from_bits_truncate(chord_note.mask[i])));

                if chord_note.slide_to[i] >= 0 {
                    slide = Some(SingleSlide {
                        to_fret: chord_note.slide_to[i] as i32,
                        slide_unpitched: false,
                    });
                }
                if chord_note.slide_unpitch_to[i] >= 0 {
                    slide = Some(SingleSlide {
                        to_fret: chord_note.slide_unpitch_to[i] as i32,
                        slide_unpitched: true,
                    });
                }

                bends = from_bend_values(
                    &chord_note.bend_data[i].bend_values[..(chord_note.bend_data[i].used_count.max(0) as usize).min(32)],
                    note.time,
                    note.sustain,
                );
                if bends.is_empty() {
                    flags.retain(|x| *x != NoteType::BEND);
                }
            }

            if note.sustain <= 0.0 {
                slide = None;
            }

            flags.sort_by_key(|x| *x as i32);
            flags.dedup();

            list.push(SingleNote {
                string_num: i as i32,
                fret_num: chord.frets[i] as i32,
                length: note.sustain as f64,
                note_type: flags,
                bends,
                slide,
            });
        }
    }

    if list.is_empty() {
        list.push(SingleNote {
            string_num: 0,
            fret_num: 0,
            length: 0.0,
            note_type: Vec::new(),
            bends: Vec::new(),
            slide: None,
        });
    }

    let mut chord_flags = Vec::new();
    if list
        .iter()
        .all(|x| x.note_type.contains(&NoteType::MUTE) || x.note_type.contains(&NoteType::FRETHANDMUTE))
    {
        chord_flags.push(NoteBlockFlags::MUTE);
    }

    NoteBlock {
        label: chord.and_then(|c| decode_zero_terminated(&c.name)),
        time: note.time as f64,
        fret_window_start: (note.anchor_fret as i32).max(1),
        fret_window_length: (note.anchor_width as i32).max(1),
        chord_flags,
        notes: list,
    }
}

fn parse_vocals_from_sng(sng: &Sng) -> Option<SongLyrics> {
    if sng.vocals.is_empty() {
        return None;
    }

    let mut lines = Vec::new();
    let mut current = Vec::new();
    for vocal in &sng.vocals {
        let raw = decode_zero_terminated(&vocal.lyric).unwrap_or_default();
        let ends_line = raw.ends_with('+');
        let text = raw.trim_end_matches('+').to_string();

        current.push(LyricWord {
            text,
            time: vocal.time as f64,
            length: vocal.length as f64,
        });

        if ends_line {
            let mut line = LyricLine {
                start_time: 0.0,
                end_time: 0.0,
                words: current,
            };
            line.refresh_timing();
            lines.push(line);
            current = Vec::new();
        }
    }

    if !current.is_empty() {
        let mut line = LyricLine {
            start_time: 0.0,
            end_time: 0.0,
            words: current,
        };
        line.refresh_timing();
        lines.push(line);
    }

    if lines.len() == 1 {
        let words = lines[0].words.clone();
        let mut block_start = f64::MAX;
        let mut buffer = Vec::new();
        let mut split = Vec::new();
        for word in words {
            if block_start == f64::MAX || word.time > block_start + 10.0 {
                block_start = word.time;
                if !buffer.is_empty() {
                    let mut line = LyricLine {
                        start_time: 0.0,
                        end_time: 0.0,
                        words: buffer,
                    };
                    line.refresh_timing();
                    split.push(line);
                    buffer = Vec::new();
                }
            }
            buffer.push(word);
        }
        if !buffer.is_empty() {
            let mut line = LyricLine {
                start_time: 0.0,
                end_time: 0.0,
                words: buffer,
            };
            line.refresh_timing();
            split.push(line);
        }
        lines = split;
    }

    Some(SongLyrics { lines })
}

fn decode_zero_terminated(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|x| *x == 0).unwrap_or(bytes.len());
    let text = String::from_utf8_lossy(&bytes[..end]).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn read_sng_any_platform(data: &[u8]) -> Option<Sng> {
    Sng::from_encrypted(data, SngPlatform::Pc)
        .ok()
        .or_else(|| Sng::from_encrypted(data, SngPlatform::Mac).ok())
        .or_else(|| Sng::read(data).ok())
}

fn from_bend_values(values: &[BendValue], note_start: f32, sustain_length: f32) -> Vec<SingleBend> {
    let mut bends = values
        .iter()
        .filter(|x| x.time > 0.0)
        .filter(|x| x.time >= note_start)
        .map(|x| SingleBend {
            step: x.step,
            time: x.time as f64,
        })
        .collect::<Vec<_>>();

    if bends.is_empty() {
        return bends;
    }

    if ((note_start + sustain_length) as f64 - bends.last().map(|x| x.time).unwrap_or(0.0)).abs() > 0.1 {
        bends.push(SingleBend {
            step: 0.0,
            time: (note_start + sustain_length) as f64,
        });
    }

    bends
}

fn convert_to_model(flag: SngNoteMask) -> Vec<NoteType> {
    let mut out = Vec::new();
    if flag.contains(SngNoteMask::CHORD) {
        out.push(NoteType::CHORD);
    }
    if flag.contains(SngNoteMask::OPEN) {
        out.push(NoteType::OPEN);
    }
    if flag.contains(SngNoteMask::FRET_HAND_MUTE) {
        out.push(NoteType::FRETHANDMUTE);
    }
    if flag.contains(SngNoteMask::TREMOLO) {
        out.push(NoteType::TREMOLO);
    }
    if flag.contains(SngNoteMask::HARMONIC) {
        out.push(NoteType::HARMONIC);
    }
    if flag.contains(SngNoteMask::PALM_MUTE) {
        out.push(NoteType::PALMMUTE);
    }
    if flag.contains(SngNoteMask::SLAP) {
        out.push(NoteType::SLAP);
    }
    if flag.contains(SngNoteMask::PLUCK) {
        out.push(NoteType::PLUCK);
    }
    if flag.contains(SngNoteMask::HAMMER_ON) {
        out.push(NoteType::HAMMERON);
    }
    if flag.contains(SngNoteMask::PULL_OFF) {
        out.push(NoteType::PULLOFF);
    }
    if flag.contains(SngNoteMask::SLIDE) {
        out.push(NoteType::SLIDE);
    }
    if flag.contains(SngNoteMask::BEND) {
        out.push(NoteType::BEND);
    }
    if flag.contains(SngNoteMask::SUSTAIN) {
        out.push(NoteType::SUSTAIN);
    }
    if flag.contains(SngNoteMask::TAP) {
        out.push(NoteType::TAP);
    }
    if flag.contains(SngNoteMask::PINCH_HARMONIC) {
        out.push(NoteType::PINCHHARMONIC);
    }
    if flag.contains(SngNoteMask::VIBRATO) {
        out.push(NoteType::VIBRATO);
    }
    if flag.contains(SngNoteMask::MUTE) {
        out.push(NoteType::MUTE);
    }
    if flag.contains(SngNoteMask::IGNORE) {
        out.push(NoteType::IGNORE);
    }
    if flag.contains(SngNoteMask::LEFT_HAND) {
        out.push(NoteType::LEFTHAND);
    }
    if flag.contains(SngNoteMask::RIGHT_HAND) {
        out.push(NoteType::RIGHTHAND);
    }
    if flag.contains(SngNoteMask::HIGH_DENSITY) {
        out.push(NoteType::HIGHDENSITY);
    }
    if flag.contains(SngNoteMask::UNPITCHED_SLIDE) {
        out.push(NoteType::SLIDEUNPITCHEDTO);
    }
    if flag.contains(SngNoteMask::SINGLE) {
        out.push(NoteType::SINGLE);
    }
    if flag.contains(SngNoteMask::CHORD_NOTES) {
        out.push(NoteType::CHORDNOTES);
    }
    if flag.contains(SngNoteMask::DOUBLE_STOP) {
        out.push(NoteType::DOUBLESTOP);
    }
    if flag.contains(SngNoteMask::ACCENT) {
        out.push(NoteType::ACCENT);
    }
    if flag.contains(SngNoteMask::PARENT) {
        out.push(NoteType::PARENT);
    }
    if flag.contains(SngNoteMask::CHILD) {
        out.push(NoteType::CHILD);
    }
    if flag.contains(SngNoteMask::ARPEGGIO) {
        out.push(NoteType::ARPEGGIO);
    }
    out
}

fn anchor_for_time(level: &rocksmith2014_xml::Level, time_ms: i32) -> (i32, i32) {
    let anchor = level
        .anchors
        .iter()
        .take_while(|x| x.time <= time_ms)
        .last()
        .or_else(|| level.anchors.first());

    if let Some(a) = anchor {
        (a.fret.max(1) as i32, a.width.max(1))
    } else {
        (1, 4)
    }
}

fn chord_notes_from_template(arrangement: &InstrumentalArrangement, chord_id: i32) -> Vec<SingleNote> {
    let Ok(idx) = usize::try_from(chord_id) else {
        return Vec::new();
    };
    let Some(template) = arrangement.chord_templates.get(idx) else {
        return Vec::new();
    };
    template_to_single_notes(template)
}

fn template_to_single_notes(template: &ChordTemplate) -> Vec<SingleNote> {
    template
        .frets
        .iter()
        .enumerate()
        .filter_map(|(string_idx, fret)| {
            if *fret < 0 {
                return None;
            }
            Some(SingleNote {
                string_num: string_idx as i32,
                fret_num: *fret as i32,
                length: 0.0,
                note_type: vec![NoteType::CHORD],
                bends: Vec::new(),
                slide: None,
            })
        })
        .collect()
}

fn instrument_name_from_path(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.strip_suffix(".xml").unwrap_or(file);
    stem.rsplit('_').next().unwrap_or("lead").to_ascii_lowercase()
}

fn instrument_name_from_sng_path(path: &str) -> Option<String> {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.strip_suffix(".sng")?;
    Some(stem.rsplit('_').next().unwrap_or("lead").to_ascii_lowercase())
}

fn instrument_order_key(name: &str) -> i32 {
    match name.to_ascii_lowercase().as_str() {
        "lead" => 0,
        "lead1" => 1,
        "lead2" => 2,
        "rhythm" => 8,
        "rhythm1" => 9,
        "rhythm2" => 10,
        "bass" => 11,
        "bass1" => 12,
        "bass2" => 13,
        _ => 999,
    }
}

fn sanitize_folder_name(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                '_'
            }
        })
        .collect::<String>()
}

fn pick_album_art_path(manifest: &[String]) -> Option<String> {
    let mut candidates = manifest
        .iter()
        .filter(|p| is_dds_path(p))
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort();

    candidates
        .iter()
        .find(|p| p.to_lowercase().contains("album"))
        .cloned()
        .or_else(|| candidates.first().cloned())
}

fn pick_audio_wem_path<R: std::io::Read + std::io::Seek>(
    psarc: &mut Psarc<R>,
    manifest: &[String],
) -> Option<String> {
    let mut wem_candidates = manifest
        .iter()
        .filter(|p| p.to_ascii_lowercase().ends_with(".wem"))
        .cloned()
        .collect::<Vec<_>>();
    wem_candidates.sort();

    if wem_candidates.is_empty() {
        return None;
    }
    if wem_candidates.len() == 1 {
        return wem_candidates.into_iter().next();
    }

    let mut bnk_candidates = manifest
        .iter()
        .filter(|p| p.to_ascii_lowercase().ends_with(".bnk"))
        .cloned()
        .collect::<Vec<_>>();
    bnk_candidates.sort_by_key(|p| {
        let lower = p.to_ascii_lowercase();
        if lower.contains("preview") {
            1
        } else {
            0
        }
    });

    let mut preview_match: Option<String> = None;
    for bnk in bnk_candidates {
        let Ok(bytes) = psarc.inflate_file(&bnk) else {
            continue;
        };
        let bnk_is_preview = bnk.to_ascii_lowercase().contains("preview");
        for wem in &wem_candidates {
            let Some(wem_id) = wem_id_from_path(wem) else {
                continue;
            };
            if bytes.windows(4).any(|w| w == wem_id.to_le_bytes()) {
                if bnk_is_preview {
                    if preview_match.is_none() {
                        preview_match = Some(wem.clone());
                    }
                } else {
                    return Some(wem.clone());
                }
            }
        }
    }

    preview_match.or_else(|| {
        wem_candidates
            .iter()
            .find(|p| !p.to_ascii_lowercase().contains("preview"))
            .cloned()
            .or_else(|| wem_candidates.first().cloned())
    })
}

fn wem_id_from_path(path: &str) -> Option<u32> {
    path.rsplit('/')
        .next()?
        .strip_suffix(".wem")?
        .parse::<u32>()
        .ok()
}

fn is_dds_path(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".dds")
}

fn upsert_song_file(list: &mut SongFileList, song_file: SongFile) {
    if let Some(existing) = list
        .data
        .iter_mut()
        .find(|x| x.folder_name == song_file.folder_name)
    {
        *existing = song_file;
    } else {
        list.data.push(song_file);
    }
}
