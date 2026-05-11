use crate::models::*;
use crate::vgmstream::{Vgmstream, VgmstreamError};
use godot::classes::Os;
use itertools::Itertools;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use rocksmith2014_psarc::Psarc;
use rocksmith2014_sng::{Platform, Sng};
use rocksmith2014_xml::InstrumentalArrangement;
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

static SONG_REPOSITORY: Lazy<Mutex<SongRepository>> = Lazy::new(|| Mutex::new(SongRepository::new()));

#[derive(Debug, thiserror::Error)]
pub enum SongRepositoryError {
    #[error("failed to load PSARC")]
    PsarcLoad,
    #[error("failed to parse PSARC file")]
    PsarcParse,
    #[error("failed to parse XML")]
    XmlParse,
    #[error("failed to parse SNG")]
    SngParse,
    #[error("failed to decode WEM audio")]
    WemDecode,
    #[error("unsupported song contents")]
    Unsupported,
}

#[derive(Clone)]
pub struct LoadedSong {
    pub id: String,
    pub info: SongInfo,
    pub audio: Vec<u8>,
    pub audio_sample_rate: i32,
    pub audio_channels: i32,
    pub source_path: PathBuf,
}

pub struct SongRepository {
    songs: Vec<LoadedSong>,
    sources: Vec<PathBuf>,
}

impl SongRepository {
    pub fn global() -> &'static Mutex<SongRepository> {
        &SONG_REPOSITORY
    }

    pub fn new() -> Self {
        let sources = read_sources_file().unwrap_or_default();
        Self {
            songs: Vec::new(),
            sources,
        }
    }

    pub fn song_files(&self) -> Vec<SongFile> {
        let mut files: Vec<SongFile> = self.songs
            .iter()
            .map(|song| {
                let instruments = song
                    .info
                    .instruments
                    .iter()
                    .enumerate()
                    .map(|(idx, inst)| SongFileInstrument {
                        name: inst.name.clone(),
                        is_main: idx == song.info.main_instrument_index,
                        tuning: inst.config.tuning,
                        note_count: inst.total_note_count(),
                        capo_fret: inst.config.capo_fret,
                    })
                    .collect();
                let lyrics = if song.info.lyrics.lines.is_empty() {
                    None
                } else {
                    Some(SongFileLyrics {
                        word_count: song
                            .info
                            .lyrics
                            .lines
                            .iter()
                            .map(|line| line.words.len())
                            .sum(),
                    })
                };

                SongFile {
                    id: song.id.clone(),
                    song_name: song.info.metadata.name.clone(),
                    artist: song.info.metadata.artist.clone(),
                    album: song.info.metadata.album.clone(),
                    year: song.info.metadata.year,
                    length: song.info.metadata.song_length,
                    instruments,
                    lyrics,
                }
            })
            .collect();
        files.sort_by(|a, b| a.song_name.cmp(&b.song_name));
        files
    }

    pub fn get_song_state(&self, id: &str, instrument: &str) -> Option<SongState> {
        let song = self.songs.iter().find(|song| song.id == id)?;
        Some(SongState {
            song_info: song.info.clone(),
            instrument_name: instrument.to_string(),
            audio: song.audio.clone(),
            audio_sample_rate: song.audio_sample_rate,
            audio_channels: song.audio_channels,
        })
    }

    pub fn reload_sources(&mut self, output: impl Fn(String)) -> Result<(), SongRepositoryError> {
        self.sources = read_sources_file().unwrap_or_default();
        self.songs.clear();
        for path in self.sources.clone() {
            output(format!("Loading {}", path.display()));
            let songs = load_psarc(&path)?;
            self.songs.extend(songs);
        }
        Ok(())
    }

    pub fn add_source(&mut self, path: &Path) -> Result<Vec<LoadedSong>, SongRepositoryError> {
        if !self.sources.contains(&path.to_path_buf()) {
            self.sources.push(path.to_path_buf());
            write_sources_file(&self.sources).map_err(|_| SongRepositoryError::PsarcLoad)?;
        }
        let songs = load_psarc(path)?;
        for song in &songs {
            if let Some(existing) = self.songs.iter_mut().find(|s| s.id == song.id) {
                *existing = song.clone();
            } else {
                self.songs.push(song.clone());
            }
        }
        Ok(songs)
    }
}

fn load_psarc(path: &Path) -> Result<Vec<LoadedSong>, SongRepositoryError> {
    let bytes = fs::read(path).map_err(|_| SongRepositoryError::PsarcLoad)?;
    let mut psarc = Psarc::read(Cursor::new(bytes)).map_err(|_| SongRepositoryError::PsarcParse)?;

    let manifest = psarc.manifest().to_vec();
    let xml_entries: Vec<String> = manifest
        .iter()
        .filter(|name| name.ends_with(".xml") && name.contains("songs/arr/"))
        .cloned()
        .collect();

    let mut arrangements_by_internal: HashMap<String, Vec<ArrangementSource>> = HashMap::new();
    for xml_path in xml_entries {
        let xml_data = psarc
            .inflate_file(&xml_path)
            .map_err(|_| SongRepositoryError::PsarcParse)?;
        let xml_str = String::from_utf8_lossy(&xml_data);
        let arrangement =
            InstrumentalArrangement::from_xml(&xml_str).map_err(|_| SongRepositoryError::XmlParse)?;
        let internal_name = arrangement.meta.internal_name.clone();
        let entry = ArrangementSource {
            internal_name: internal_name.clone(),
            arrangement_name: arrangement.meta.arrangement.clone(),
            xml_path,
            meta: arrangement.meta,
        };
        arrangements_by_internal
            .entry(internal_name)
            .or_default()
            .push(entry);
    }

    let mut songs = Vec::new();
    for (internal_name, arrangements) in arrangements_by_internal {
        if arrangements.is_empty() {
            continue;
        }
        let song_meta = &arrangements[0].meta;
        let metadata = SongMetadata::new(
            song_meta.song_name.clone(),
            song_meta.artist_name.clone(),
            song_meta.album_name.clone(),
            if song_meta.album_year == 0 {
                None
            } else {
                Some(song_meta.album_year)
            },
            (song_meta.song_length as f32) / 1000.0,
            None,
        );

        let instruments = arrangements
            .iter()
            .filter_map(|arr| {
                let arrangement_name = normalize_arrangement_name(&arr.arrangement_name);
                let sng_path = find_sng_path(&manifest, &internal_name, &arrangement_name)?;
                let encrypted = psarc
                    .inflate_file(&sng_path)
                    .map_err(|_| SongRepositoryError::PsarcParse)
                    .ok()?;
                let sng = Sng::from_encrypted(&encrypted, Platform::Pc)
                    .map_err(|_| SongRepositoryError::SngParse)
                    .ok()?;
                let tuning = song_meta.tuning.strings;
                let config = InstrumentConfig {
                    note_speed: 40.0,
                    tuning,
                    capo_fret: song_meta.capo as f32,
                    cent_offset: song_meta.cent_offset as f32,
                };
                Some(convert_instrument(arrangement_name, &sng, config))
            })
            .collect::<Vec<_>>();

        if instruments.is_empty() {
            continue;
        }

        let lyrics = load_lyrics(&arrangements, &mut psarc, &manifest, &internal_name);
        let song_info = SongInfo::new(metadata, instruments, lyrics);

        let wem_entries = manifest
            .iter()
            .enumerate()
            .filter_map(|(idx, name)| {
                if name.ends_with(".wem") {
                    let size = psarc.toc().get(idx).map(|entry| entry.length).unwrap_or(0);
                    Some((name.clone(), size))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let wem_path = select_wem_path(&wem_entries, &internal_name);
        let wem_data = wem_path
            .as_deref()
            .and_then(|path| psarc.inflate_file(path).ok())
            .ok_or(SongRepositoryError::WemDecode)?;

        let decoded = Vgmstream::load_default()
            .map_err(|_| SongRepositoryError::WemDecode)?
            .decode_wem(&wem_data)
            .map_err(|_| SongRepositoryError::WemDecode)?;

        let id = sanitize_id(&song_info.metadata.artist, &song_info.metadata.name);
        songs.push(LoadedSong {
            id,
            info: song_info,
            audio: decoded.data,
            audio_sample_rate: decoded.sample_rate,
            audio_channels: decoded.channels,
            source_path: path.to_path_buf(),
        });
    }

    Ok(songs)
}

fn find_sng_path(manifest: &[String], internal_name: &str, arrangement: &str) -> Option<String> {
    let arrangement_key = format!("{}_{}", internal_name, arrangement).to_lowercase();
    manifest
        .iter()
        .find(|name| {
            let lower = name.to_lowercase();
            lower.ends_with(".sng") && lower.contains(&arrangement_key)
        })
        .cloned()
        .or_else(|| {
            manifest
                .iter()
                .find(|name| {
                    let lower = name.to_lowercase();
                    lower.ends_with(".sng") && lower.contains(&internal_name.to_lowercase())
                })
                .cloned()
        })
}

fn select_wem_path(entries: &[(String, u64)], internal_name: &str) -> Option<String> {
    let mut candidates = entries.to_vec();
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    let internal = internal_name.to_lowercase();
    candidates
        .iter()
        .find(|(name, _)| name.to_lowercase().contains(&internal))
        .map(|(name, _)| name.clone())
        .or_else(|| candidates.first().map(|(name, _)| name.clone()))
}

fn load_lyrics(
    arrangements: &[ArrangementSource],
    psarc: &mut Psarc<Cursor<Vec<u8>>>,
    manifest: &[String],
    internal_name: &str,
) -> Lyrics {
    let vocals_path = manifest.iter().find(|name| {
        let lower = name.to_lowercase();
        lower.ends_with(".sng") && lower.contains("vocals") && lower.contains(&internal_name.to_lowercase())
    });
    if let Some(path) = vocals_path {
        if let Ok(encrypted) = psarc.inflate_file(path) {
            if let Ok(sng) = Sng::from_encrypted(&encrypted, Platform::Pc) {
                return convert_vocals(&sng);
            }
        }
    }

    for arr in arrangements {
        if arr.arrangement_name.to_lowercase().contains("vocals") {
            let sng_path = find_sng_path(manifest, &arr.internal_name, "vocals");
            if let Some(path) = sng_path {
                if let Ok(encrypted) = psarc.inflate_file(&path) {
                    if let Ok(sng) = Sng::from_encrypted(&encrypted, Platform::Pc) {
                        return convert_vocals(&sng);
                    }
                }
            }
        }
    }

    Lyrics::default()
}

fn convert_vocals(sng: &Sng) -> Lyrics {
    let mut lines = Vec::new();
    let mut current = Vec::new();
    for vocal in &sng.vocals {
        let text = bytes_to_string(&vocal.lyric);
        if text.is_empty() {
            continue;
        }
        current.push(Lyric {
            text: text.trim_end_matches('+').to_string(),
            time: vocal.time,
            length: vocal.length,
        });
        if text.ends_with('+') {
            if !current.is_empty() {
                lines.push(LyricLine::new(current));
                current = Vec::new();
            }
        }
    }
    if !current.is_empty() {
        lines.push(LyricLine::new(current));
    }
    Lyrics { lines }
}

fn bytes_to_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn normalize_arrangement_name(name: &str) -> String {
    name.to_lowercase().replace(' ', "")
}

fn convert_instrument(name: String, sng: &Sng, config: InstrumentConfig) -> Instrument {
    let mut all_notes = Vec::new();
    for (idx, _) in sng.phrase_iterations.iter().enumerate() {
        for level in sng.levels.iter().sorted_by(|a, b| b.difficulty.cmp(&a.difficulty)) {
            let notes = level
                .notes
                .iter()
                .filter(|note| note.phrase_iteration_id == idx as i32)
                .cloned()
                .collect::<Vec<_>>();
            if !notes.is_empty() {
                all_notes.extend(notes);
                break;
            }
        }
    }

    let mut note_blocks: Vec<NoteBlock> = all_notes
        .iter()
        .map(|note| convert_note_block(note, sng))
        .collect();

    let grouped_times = note_blocks
        .iter()
        .filter(|n| !n.is_chord())
        .map(|n| n.time)
        .collect::<Vec<_>>();
    for time in grouped_times.iter().unique() {
        let same_time: Vec<NoteBlock> = note_blocks
            .iter()
            .filter(|n| !n.is_chord() && (n.time - *time).abs() < f32::EPSILON)
            .cloned()
            .collect();
        if same_time.len() > 1 {
            note_blocks.retain(|n| !(n.time - *time).abs() < f32::EPSILON || n.is_chord());
            let notes = same_time
                .iter()
                .cloned()
                .flat_map(|block| block.notes)
                .collect::<Vec<_>>();
            let fret_window_start = same_time
                .first()
                .map(|n| n.fret_window_start)
                .unwrap_or(0);
            let fret_window_length = same_time
                .first()
                .map(|n| n.fret_window_length)
                .unwrap_or(4);
            note_blocks.push(NoteBlock::new(
                *time,
                fret_window_start,
                fret_window_length,
                notes,
                Vec::new(),
            ));
        }
    }

    let sections = sng
        .phrase_iterations
        .iter()
        .filter_map(|pi| {
            let phrase = sng.phrases.get(pi.phrase_id as usize)?;
            let name = bytes_to_string(&phrase.name);
            Some(Section {
                start_time: pi.start_time,
                end_time: pi.end_time,
                name,
            })
        })
        .collect();

    Instrument::new(name, note_blocks, sections, config)
}

fn convert_note_block(note: &rocksmith2014_sng::Note, sng: &Sng) -> NoteBlock {
    let fret_window_start = note.anchor_fret as i32;
    let fret_window_length = note.anchor_width as i32;
    let note_types = convert_note_mask(note.mask);
    let slide = slide_from_note(note);
    let bends = convert_bends(&note.bend_data, note.time, note.sustain);

    if note.mask.contains(rocksmith2014_sng::NoteMask::CHORD) {
        let chord = note
            .chord_id
            .try_into()
            .ok()
            .and_then(|idx: usize| sng.chords.get(idx));
        let chord_notes = note
            .chord_notes_id
            .try_into()
            .ok()
            .and_then(|idx: usize| sng.chord_notes.get(idx));

        let mut notes = Vec::new();
        let mut chord_flags = Vec::new();
        if let Some(chord) = chord {
            for (string_idx, fret) in chord.frets.iter().enumerate() {
                if *fret < 0 {
                    continue;
                }
                let mut note_mask_types = note_types.clone();
                let mut local_bends = bends.clone();
                let mut local_slide = slide.clone();
                if let Some(chord_notes) = chord_notes {
                    if let Some(mask) = chord_notes.mask.get(string_idx) {
                        note_mask_types.extend(convert_note_mask(rocksmith2014_sng::NoteMask::from_bits_truncate(
                            *mask,
                        )));
                    }
                    if let Some(bend_data) = chord_notes.bend_data.get(string_idx) {
                        local_bends = convert_bends(&bend_data.bend_data, note.time, note.sustain);
                    }
                    if let Some(slide_to) = chord_notes.slide_to.get(string_idx) {
                        if *slide_to >= 0 {
                            local_slide = Some(SingleSlide {
                                to_fret: *slide_to as i32,
                                slide_unpitched: false,
                            });
                        }
                    }
                    if let Some(slide_to) = chord_notes.slide_unpitch_to.get(string_idx) {
                        if *slide_to >= 0 {
                            local_slide = Some(SingleSlide {
                                to_fret: *slide_to as i32,
                                slide_unpitched: true,
                            });
                        }
                    }
                }
                notes.push(SingleNote::new(
                    string_idx as i32,
                    *fret as i32,
                    note.sustain,
                    note_mask_types.clone(),
                    local_bends.clone(),
                    local_slide.clone(),
                ));
            }
            if notes.iter().all(|n| n.types.contains(&NoteType::Mute)) {
                chord_flags.push(NoteBlockFlags::Mute);
            }
        }

        let mut block = NoteBlock::new(note.time, fret_window_start, fret_window_length, notes, chord_flags);
        if let Some(chord) = chord {
            let label = bytes_to_string(&chord.name);
            if !label.is_empty() {
                block.label = Some(label);
            }
        }
        return block;
    }

    NoteBlock::new(
        note.time,
        fret_window_start,
        fret_window_length,
        vec![SingleNote::new(
            note.string_index as i32,
            note.fret as i32,
            note.sustain,
            note_types,
            bends,
            slide,
        )],
        Vec::new(),
    )
}

fn slide_from_note(note: &rocksmith2014_sng::Note) -> Option<SingleSlide> {
    if note.slide_to >= 0 {
        Some(SingleSlide {
            to_fret: note.slide_to as i32,
            slide_unpitched: false,
        })
    } else if note.slide_unpitch_to >= 0 {
        Some(SingleSlide {
            to_fret: note.slide_unpitch_to as i32,
            slide_unpitched: true,
        })
    } else {
        None
    }
}

fn convert_bends(
    bends: &[rocksmith2014_sng::BendValue],
    note_start: f32,
    sustain_length: f32,
) -> Option<Vec<SingleBend>> {
    if bends.is_empty() {
        return None;
    }
    let mut actual: Vec<SingleBend> = bends
        .iter()
        .filter(|bend| bend.time >= note_start)
        .map(|bend| SingleBend {
            step: bend.step as f32,
            time: bend.time,
        })
        .collect();
    if actual.is_empty() {
        return None;
    }
    if let Some(last) = actual.last() {
        if (note_start + sustain_length - last.time).abs() > 0.1 {
            actual.push(SingleBend {
                step: 0.0,
                time: note_start + sustain_length,
            });
        }
    }
    Some(actual)
}

fn convert_note_mask(mask: rocksmith2014_sng::NoteMask) -> Vec<NoteType> {
    let mut out = Vec::new();
    if mask.contains(rocksmith2014_sng::NoteMask::CHORD) {
        out.push(NoteType::Chord);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::OPEN) {
        out.push(NoteType::Open);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::FRET_HAND_MUTE) {
        out.push(NoteType::FretHandMute);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::TREMOLO) {
        out.push(NoteType::Tremolo);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::HARMONIC) {
        out.push(NoteType::Harmonic);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::PALM_MUTE) {
        out.push(NoteType::PalmMute);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::SLAP) {
        out.push(NoteType::Slap);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::PLUCK) {
        out.push(NoteType::Pluck);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::PULL_OFF) {
        out.push(NoteType::PullOff);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::HAMMER_ON) {
        out.push(NoteType::HammerOn);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::SLIDE) {
        out.push(NoteType::Slide);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::BEND) {
        out.push(NoteType::Bend);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::SUSTAIN) {
        out.push(NoteType::Sustain);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::TAP) {
        out.push(NoteType::Tap);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::PINCH_HARMONIC) {
        out.push(NoteType::PinchHarmonic);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::VIBRATO) {
        out.push(NoteType::Vibrato);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::MUTE) {
        out.push(NoteType::Mute);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::IGNORE) {
        out.push(NoteType::Ignore);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::LEFT_HAND) {
        out.push(NoteType::LeftHand);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::RIGHT_HAND) {
        out.push(NoteType::RightHand);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::HIGH_DENSITY) {
        out.push(NoteType::HighDensity);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::UNPITCHED_SLIDE) {
        out.push(NoteType::SlideUnpitchedTo);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::SINGLE) {
        out.push(NoteType::Single);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::CHORD_NOTES) {
        out.push(NoteType::ChordNotes);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::DOUBLE_STOP) {
        out.push(NoteType::DoubleStop);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::ACCENT) {
        out.push(NoteType::Accent);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::PARENT) {
        out.push(NoteType::Parent);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::CHILD) {
        out.push(NoteType::Child);
    }
    if mask.contains(rocksmith2014_sng::NoteMask::ARPEGGIO) {
        out.push(NoteType::Arpeggio);
    }
    out
}

fn sanitize_id(artist: &str, song: &str) -> String {
    let mut id = format!("{}_{}", artist, song)
        .replace('/', "-")
        .replace(' ', "-");
    id.retain(|c| !c.is_ascii_control());
    id
}

fn sources_file_path() -> PathBuf {
    let os = Os::singleton();
    let base = os.get_user_data_dir().to_string();
    PathBuf::from(base).join("song_sources.json")
}

fn read_sources_file() -> Option<Vec<PathBuf>> {
    let path = sources_file_path();
    let data = fs::read_to_string(path).ok()?;
    let list: Vec<String> = serde_json::from_str(&data).ok()?;
    Some(list.into_iter().map(PathBuf::from).collect())
}

fn write_sources_file(paths: &[PathBuf]) -> Result<(), std::io::Error> {
    let path = sources_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let list: Vec<String> = paths.iter().map(|p| p.to_string_lossy().to_string()).collect();
    fs::write(path, serde_json::to_string_pretty(&list).unwrap_or_default())
}

struct ArrangementSource {
    internal_name: String,
    arrangement_name: String,
    xml_path: String,
    meta: rocksmith2014_xml::MetaData,
}

impl From<VgmstreamError> for SongRepositoryError {
    fn from(_: VgmstreamError) -> Self {
        SongRepositoryError::WemDecode
    }
}
