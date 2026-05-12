use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rocksmith2014_psarc::Psarc;
use rocksmith2014_sng::{BendValue, Note as SngNote, NoteMask as SngNoteMask, Platform as SngPlatform, Sng};
use rocksmith2014_xml::{ChordTemplate, InstrumentalArrangement};
use tabplayer_ffi::native_audio::{decode_wem_to_pcm, encode_ogg_48k};

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

    let song_dir = song_root_folder().join(&folder_name);
    fs::create_dir_all(&song_dir)?;

    let copied_psarc = song_dir.join(
        path.file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("song.psarc"),
    );
    fs::copy(path, copied_psarc).with_context(|| format!("failed copying '{}'", path.display()))?;

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

    if arrangement_paths.is_empty() {
        anyhow::bail!("no arrangement xml entries in '{}'", path.display());
    }

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

    let (_, primary) = parsed_arrangements
        .first()
        .ok_or_else(|| anyhow::anyhow!("no parsed arrangements for '{}'", path.display()))?;

    let metadata = SongMetadata {
        name: if primary.meta.song_name.is_empty() {
            file_stem.replace('_', " ")
        } else {
            primary.meta.song_name.clone()
        },
        artist: if primary.meta.artist_name.is_empty() {
            "Unknown Artist".to_string()
        } else {
            primary.meta.artist_name.clone()
        },
        album: if primary.meta.album_name.is_empty() {
            "Unknown Album".to_string()
        } else {
            primary.meta.album_name.clone()
        },
        year: if primary.meta.album_year > 0 {
            Some(primary.meta.album_year)
        } else {
            None
        },
        song_length: if primary.meta.song_length > 0 {
            primary.meta.song_length as f64 / 1000.0
        } else {
            0.0
        },
    };

    let mut instruments = Vec::new();
    for (arrangement_path, arrangement) in parsed_arrangements {
        let instrument_name = instrument_name_from_path(&arrangement_path);
        let instrument = convert_instrument(&instrument_name, &arrangement, sng_by_instrument.get(&instrument_name));
        instruments.push(instrument);
    }
    instruments.sort_by_key(|x| instrument_order_key(&x.name));

    let lyrics = vocals_sng.as_ref().and_then(parse_vocals_from_sng);

    let song_data = SongData {
        metadata: metadata.clone(),
        instruments: instruments.clone(),
        lyrics,
    };

    let data_path = song_dir.join("data.json");
    fs::write(&data_path, serde_json::to_vec_pretty(&song_data)?)?;

    if let Some(art_path) = pick_album_art_path(&manifest) {
        if let Ok(bytes) = psarc.inflate_file(&art_path) {
            let art_file = song_dir.join("album.dds");
            let _ = fs::write(art_file, bytes);
        }
    }

    if let Some(wem_path) = pick_audio_wem_path(&manifest) {
        if let Ok(bytes) = psarc.inflate_file(&wem_path) {
            let wem_file = song_dir.join("song.wem");
            let _ = fs::write(&wem_file, bytes);
            let ogg_file = song_dir.join("song.ogg");
            match decode_wem_to_pcm(&wem_file) {
                Ok(decoded) => {
                    if let Err(err) = encode_ogg_48k(&decoded, &ogg_file) {
                        eprintln!("[audio] failed encoding ogg for '{}': {err}", wem_file.display());
                    }
                }
                Err(err) => {
                    eprintln!("[audio] failed decoding wem '{}': {err}", wem_file.display());
                }
            }
        }
    }

    let song_file = SongFile {
        folder_name: folder_name.clone(),
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

    Ok((
        song_file,
        ImportEntry {
            source_path: path.to_path_buf(),
            folder_name,
        },
    ))
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

fn pick_audio_wem_path(manifest: &[String]) -> Option<String> {
    let mut candidates = manifest
        .iter()
        .filter(|p| p.to_ascii_lowercase().ends_with(".wem"))
        .filter(|p| {
            let lower = p.to_ascii_lowercase();
            !lower.contains("preview") && !lower.contains("_p")
        })
        .cloned()
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        candidates = manifest
            .iter()
            .filter(|p| p.to_ascii_lowercase().ends_with(".wem"))
            .cloned()
            .collect::<Vec<_>>();
    }

    candidates.sort();
    candidates.into_iter().next()
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
