use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct SongInfo {
    pub metadata: SongMetadata,
    pub instruments: Vec<Instrument>,
    pub lyrics: Lyrics,
    pub main_instrument_index: usize,
}

impl SongInfo {
    pub const LEAD_NAME: &'static str = "lead";
    pub const LEAD1_NAME: &'static str = "lead1";
    pub const LEAD2_NAME: &'static str = "lead2";
    pub const COMBO_NAME: &'static str = "combo";
    pub const COMBO1_NAME: &'static str = "combo1";
    pub const COMBO2_NAME: &'static str = "combo2";
    pub const COMBO3_NAME: &'static str = "combo3";
    pub const COMBO4_NAME: &'static str = "combo4";
    pub const RHYTHM_NAME: &'static str = "rhythm";
    pub const RHYTHM1_NAME: &'static str = "rhythm1";
    pub const RHYTHM2_NAME: &'static str = "rhythm2";
    pub const BASS_NAME: &'static str = "bass";
    pub const BASS1_NAME: &'static str = "bass1";
    pub const BASS2_NAME: &'static str = "bass2";
    pub const VOCALS_NAME: &'static str = "vocals";

    pub const STANDARD_INSTRUMENT_TYPES: [&'static str; 3] =
        [Self::LEAD_NAME, Self::RHYTHM_NAME, Self::BASS_NAME];

    pub fn new(metadata: SongMetadata, instruments: Vec<Instrument>, lyrics: Lyrics) -> Self {
        let mut info = Self {
            metadata,
            instruments,
            lyrics,
            main_instrument_index: 0,
        };
        info.main_instrument_index = info.find_main_instrument_index();
        info.ensure_lyrics_lines();
        info
    }

    pub fn main_instrument(&self) -> &Instrument {
        &self.instruments[self.main_instrument_index]
    }

    pub fn main_instrument_mut(&mut self) -> &mut Instrument {
        &mut self.instruments[self.main_instrument_index]
    }

    fn find_main_instrument_index(&self) -> usize {
        let preferred = [
            Self::LEAD_NAME,
            Self::LEAD1_NAME,
            Self::LEAD2_NAME,
            Self::RHYTHM_NAME,
            Self::RHYTHM1_NAME,
            Self::RHYTHM2_NAME,
            Self::BASS_NAME,
            Self::BASS1_NAME,
            Self::BASS2_NAME,
            Self::COMBO_NAME,
            Self::COMBO1_NAME,
            Self::COMBO2_NAME,
            Self::COMBO3_NAME,
            Self::COMBO4_NAME,
        ];
        for name in preferred.iter() {
            if let Some((idx, _)) = self
                .instruments
                .iter()
                .enumerate()
                .find(|(_, inst)| inst.name == *name)
            {
                return idx;
            }
        }
        if self.instruments.is_empty() {
            0
        } else {
            0
        }
    }

    fn ensure_lyrics_lines(&mut self) {
        if self.lyrics.lines.len() != 1 {
            return;
        }

        let words = self.lyrics.lines[0].words.clone();
        if words.is_empty() {
            return;
        }

        let mut lines = Vec::new();
        let mut block_start_time = f32::MAX;
        let mut current_words: Vec<Lyric> = Vec::new();
        for word in words {
            if block_start_time == f32::MAX || word.time > block_start_time + 10.0 {
                if !current_words.is_empty() {
                    lines.push(LyricLine::new(current_words));
                }
                block_start_time = word.time;
                current_words = Vec::new();
            }
            current_words.push(word);
        }

        if !current_words.is_empty() {
            lines.push(LyricLine::new(current_words));
        }

        self.lyrics = Lyrics { lines };
    }
}

#[derive(Debug, Clone)]
pub struct SongMetadata {
    pub name: String,
    pub artist: String,
    pub album: String,
    pub year: Option<i32>,
    pub song_length: f32,
    pub genre: Option<String>,
}

impl SongMetadata {
    pub fn new(
        name: String,
        artist: String,
        album: String,
        year: Option<i32>,
        song_length: f32,
        genre: Option<String>,
    ) -> Self {
        Self {
            name,
            artist,
            album,
            year,
            song_length,
            genre,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InstrumentConfig {
    pub note_speed: f32,
    pub tuning: [i16; 6],
    pub capo_fret: f32,
    pub cent_offset: f32,
}

#[derive(Debug, Clone)]
pub struct Instrument {
    pub name: String,
    pub notes: Vec<NoteBlock>,
    pub sections: Vec<Section>,
    pub config: InstrumentConfig,
    pub control_data: SimulationControlData,
}

impl Instrument {
    pub fn new(
        name: String,
        notes: Vec<NoteBlock>,
        sections: Vec<Section>,
        config: InstrumentConfig,
    ) -> Self {
        let mut notes_sorted = notes;
        notes_sorted.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(Ordering::Equal));
        let control_data = SimulationControlData::calc_control_data(&notes_sorted);
        Self {
            name,
            notes: notes_sorted,
            sections,
            config,
            control_data,
        }
    }

    pub fn total_note_count(&self) -> usize {
        self.notes.len()
    }

    pub fn chord_count(&self) -> usize {
        self.notes.iter().filter(|n| n.is_chord()).count()
    }

    pub fn single_note_count(&self) -> usize {
        self.notes.iter().filter(|n| !n.is_chord()).count()
    }

    pub fn last_note_time(&self) -> f32 {
        self.notes.last().map(|n| n.time).unwrap_or(0.0)
    }

    pub fn get_note_density(&self, song_info: &SongInfo) -> f32 {
        if song_info.metadata.song_length <= 0.0 {
            return 0.0;
        }
        self.total_note_count() as f32 / song_info.metadata.song_length
    }

    pub fn calc_tuning_name(tuning: [i16; 6], capo_fret: f32) -> String {
        if capo_fret != 0.0 && capo_fret != 255.0 {
            return format!("{}, Capo: {}", Self::calc_tuning_name_without_capo(tuning), capo_fret);
        }
        Self::calc_tuning_name_without_capo(tuning)
    }

    pub fn calc_tuning_name_without_capo(tuning: [i16; 6]) -> String {
        if tuning == [1, 1, 1, 1, 1, 1] {
            return "F Standard".to_string();
        }
        if tuning == [0, 0, 0, 0, 0, 0] {
            return "E Standard".to_string();
        }
        if tuning == [-2, 0, 0, 0, 0, 0] {
            return "Drop D".to_string();
        }
        if tuning == [-1, -1, -1, -1, -1, -1] {
            return "Eb Standard".to_string();
        }
        if tuning == [-3, -1, -1, -1, -1, -1] {
            return "Eb Drop Db".to_string();
        }
        if tuning == [-2, -2, -2, -2, -2, -2] {
            return "D Standard".to_string();
        }
        if tuning == [-4, -2, -2, -2, -2, -2] {
            return "D Drop C".to_string();
        }
        if tuning == [-3, -3, -3, -3, -3, -3] {
            return "Db Standard".to_string();
        }
        if tuning == [-5, -3, -3, -3, -3, -3] {
            return "Db Drop B".to_string();
        }
        if tuning == [-4, -4, -4, -4, -4, -4] {
            return "C Standard".to_string();
        }
        if tuning == [-6, -4, -4, -4, -4, -4] {
            return "C Drop Bb".to_string();
        }
        if tuning == [-5, -5, -5, -5, -5, -5] {
            return "B Standard".to_string();
        }

        tuning
            .iter()
            .enumerate()
            .map(|(idx, offset)| note_for_index_and_offset(idx, *offset))
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn note_for_index_and_offset(index: usize, offset: i16) -> String {
    const NOTE_OFFSET: [i32; 6] = [0, 5, 10, 3, 7, 0];
    const NOTE_LIST: [&str; 12] = [
        "E", "F", "Gb", "G", "Ab", "A", "Bb", "B", "C", "Db", "D", "Eb",
    ];
    let mut pos = NOTE_OFFSET[index] + offset as i32;
    while pos < 0 {
        pos += NOTE_LIST.len() as i32;
    }
    let result = NOTE_LIST[(pos as usize) % NOTE_LIST.len()];
    if index == 5 {
        result.to_lowercase()
    } else {
        result.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct SongFile {
    pub id: String,
    pub song_name: String,
    pub artist: String,
    pub album: String,
    pub year: Option<i32>,
    pub length: f32,
    pub instruments: Vec<SongFileInstrument>,
    pub lyrics: Option<SongFileLyrics>,
}

impl SongFile {
    pub fn instrument_chars(&self) -> String {
        let mut chars = [' ', ' ', ' ', ' ', ' '];
        if self
            .instruments
            .iter()
            .any(|x| matches!(x.name.as_str(), SongInfo::LEAD_NAME | SongInfo::LEAD1_NAME | SongInfo::LEAD2_NAME))
        {
            chars[0] = 'L';
        }
        if self.instruments.iter().any(|x| {
            matches!(
                x.name.as_str(),
                SongInfo::RHYTHM_NAME | SongInfo::RHYTHM1_NAME | SongInfo::RHYTHM2_NAME
            )
        }) {
            chars[1] = 'R';
        }
        if self
            .instruments
            .iter()
            .any(|x| matches!(x.name.as_str(), SongInfo::BASS_NAME | SongInfo::BASS1_NAME | SongInfo::BASS2_NAME))
        {
            chars[2] = 'B';
        }
        if let Some(lyrics) = &self.lyrics {
            if lyrics.word_count > 0 {
                chars[3] = 'V';
            }
        }
        let other_instruments = self
            .instruments
            .iter()
            .filter(|x| !SongInfo::STANDARD_INSTRUMENT_TYPES.contains(&x.name.as_str()))
            .count();
        if other_instruments > 0 {
            chars[4] = other_instruments.to_string().chars().next().unwrap_or(' ');
        }
        chars.iter().collect()
    }

    pub fn main_instrument(&self) -> Option<&SongFileInstrument> {
        self.instruments.iter().find(|x| x.is_main)
    }
}

#[derive(Debug, Clone)]
pub struct SongFileInstrument {
    pub name: String,
    pub is_main: bool,
    pub tuning: [i16; 6],
    pub note_count: usize,
    pub capo_fret: f32,
}

impl SongFileInstrument {
    pub fn note_density(&self, song: &SongFile) -> f32 {
        if song.length <= 0.0 {
            0.0
        } else {
            self.note_count as f32 / song.length
        }
    }
}

#[derive(Debug, Clone)]
pub struct SongFileLyrics {
    pub word_count: usize,
}

#[derive(Debug, Clone)]
pub struct SongState {
    pub song_info: SongInfo,
    pub instrument_name: String,
    pub audio: Vec<u8>,
    pub audio_sample_rate: i32,
    pub audio_channels: i32,
}

impl SongState {
    pub fn instrument(&self) -> &Instrument {
        self.song_info
            .instruments
            .iter()
            .find(|inst| inst.name == self.instrument_name)
            .unwrap_or_else(|| self.song_info.main_instrument())
    }
}

#[derive(Debug, Clone)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
}

impl Default for Lyrics {
    fn default() -> Self {
        Self { lines: Vec::new() }
    }
}

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub words: Vec<Lyric>,
    pub start_time: f32,
    pub end_time: f32,
}

impl LyricLine {
    pub fn new(words: Vec<Lyric>) -> Self {
        let start_time = words.first().map(|w| w.time).unwrap_or(0.0);
        let end_time = words
            .last()
            .map(|w| w.time + w.length)
            .unwrap_or(0.0);
        Self {
            words,
            start_time,
            end_time,
        }
    }

    pub fn text(&self) -> String {
        let mut output = String::new();
        for word in &self.words {
            output.push_str(&word.text);
            if !word.text.ends_with('-') {
                output.push(' ');
            }
        }
        output
    }

    pub fn get_parts(&self, song_position: f64) -> (String, String) {
        let mut part_a = String::new();
        let mut part_b = String::new();
        for word in &self.words {
            if (word.time as f64) <= song_position {
                part_a.push_str(&word.text);
                if !word.text.ends_with('-') {
                    part_a.push(' ');
                }
            } else {
                part_b.push_str(&word.text);
                if !word.text.ends_with('-') {
                    part_b.push(' ');
                }
            }
        }
        (part_a, part_b)
    }
}

#[derive(Debug, Clone)]
pub struct Lyric {
    pub text: String,
    pub time: f32,
    pub length: f32,
}

#[derive(Debug, Clone)]
pub struct NoteBlock {
    pub label: Option<String>,
    pub time: f32,
    pub fret_window_start: i32,
    pub fret_window_length: i32,
    pub chord_flags: Vec<NoteBlockFlags>,
    pub notes: Vec<SingleNote>,
}

impl NoteBlock {
    pub fn new(
        time: f32,
        fret_window_start: i32,
        fret_window_length: i32,
        notes: Vec<SingleNote>,
        chord_flags: Vec<NoteBlockFlags>,
    ) -> Self {
        let mut length = fret_window_length;
        if length < 1 {
            length = 4;
        }
        Self {
            label: None,
            time,
            fret_window_start,
            fret_window_length: length,
            notes,
            chord_flags,
        }
    }

    pub fn is_chord(&self) -> bool {
        self.notes.len() > 1
    }

    pub fn is_same_chord_as(&self, other: Option<&NoteBlock>) -> bool {
        let other = match other {
            Some(other) => other,
            None => return false,
        };
        let this_has_stuff = self
            .notes
            .iter()
            .any(|n| (n.bends.as_ref().map_or(false, |b| !b.is_empty())) || n.slide.is_some());
        if this_has_stuff {
            return false;
        }
        let other_has_stuff = other
            .notes
            .iter()
            .any(|n| (n.bends.as_ref().map_or(false, |b| !b.is_empty())) || n.slide.is_some());
        if other_has_stuff {
            return false;
        }
        if self.notes.len() != other.notes.len() {
            return false;
        }
        if self.chord_flags != other.chord_flags {
            return false;
        }
        for (left, right) in self.notes.iter().zip(other.notes.iter()) {
            if left.fret_num != right.fret_num || left.string_num != right.string_num {
                return false;
            }
            let types_left = left
                .types
                .iter()
                .filter(|t| !NoteType::ignored_types().contains(t))
                .collect::<Vec<_>>();
            let types_right = right
                .types
                .iter()
                .filter(|t| !NoteType::ignored_types().contains(t))
                .collect::<Vec<_>>();
            if types_left != types_right {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteBlockFlags {
    Undefined,
    Mute,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NoteType {
    Undefined,
    Missing,
    Chord,
    Open,
    FretHandMute,
    Tremolo,
    Harmonic,
    PalmMute,
    Slap,
    Pluck,
    Pop,
    HammerOn,
    PullOff,
    Slide,
    Bend,
    Sustain,
    Tap,
    PinchHarmonic,
    Vibrato,
    Mute,
    Ignore,
    LeftHand,
    RightHand,
    HighDensity,
    SlideUnpitchedTo,
    Single,
    ChordNotes,
    DoubleStop,
    Accent,
    Parent,
    Child,
    Arpeggio,
    Missing2,
    Strum,
}

impl NoteType {
    pub fn ignored_types() -> &'static [NoteType] {
        &[
            NoteType::Undefined,
            NoteType::Missing,
            NoteType::Chord,
            NoteType::Open,
            NoteType::Ignore,
            NoteType::HighDensity,
            NoteType::Single,
            NoteType::ChordNotes,
            NoteType::DoubleStop,
            NoteType::Missing2,
            NoteType::Strum,
            NoteType::Accent,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct SingleNote {
    pub string_num: i32,
    pub fret_num: i32,
    pub length: f32,
    pub types: Vec<NoteType>,
    pub bends: Option<Vec<SingleBend>>,
    pub slide: Option<SingleSlide>,
}

impl SingleNote {
    pub fn new(
        string_num: i32,
        fret_num: i32,
        length: f32,
        types: Vec<NoteType>,
        bends: Option<Vec<SingleBend>>,
        slide: Option<SingleSlide>,
    ) -> Self {
        let slide = if slide.is_some() && length <= 0.0 {
            None
        } else {
            slide
        };
        Self {
            string_num,
            fret_num,
            length,
            types,
            bends,
            slide,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SingleBend {
    pub step: f32,
    pub time: f32,
}

#[derive(Debug, Clone)]
pub struct SingleSlide {
    pub to_fret: i32,
    pub slide_unpitched: bool,
}

#[derive(Debug, Clone)]
pub struct SimulationControlData {
    pub note_center_fret: Vec<CenterFret>,
}

impl SimulationControlData {
    fn calc_control_data(notes: &[NoteBlock]) -> Self {
        let mut note_center_fret = Vec::new();
        let mut last_fret = -1;
        let mut last_width = -1;
        for note in notes {
            if last_fret != note.fret_window_start || last_width != note.fret_window_length {
                note_center_fret.push(CenterFret {
                    time: note.time,
                    fret: note.fret_window_start,
                    window_size: note.fret_window_length,
                });
                last_fret = note.fret_window_start;
                last_width = note.fret_window_length;
            }
        }
        Self { note_center_fret }
    }
}

#[derive(Debug, Clone)]
pub struct CenterFret {
    pub time: f32,
    pub fret: i32,
    pub window_size: i32,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub start_time: f32,
    pub end_time: f32,
    pub name: String,
}

pub fn note_symbols(note: &SingleNote) -> Vec<String> {
    let mut symbols = Vec::new();
    if note.types.contains(&NoteType::HammerOn) {
        symbols.push("h".to_string());
    }
    if note.types.contains(&NoteType::PullOff) {
        symbols.push("p".to_string());
    }
    if note.types.contains(&NoteType::Bend) {
        if let Some(bends) = &note.bends {
            if let Some(max) = bends.iter().max_by(|a, b| a.step.partial_cmp(&b.step).unwrap_or(Ordering::Equal)) {
                symbols.push(format!("b{}", max.step as i32));
            }
        }
    }
    if note.types.contains(&NoteType::LeftHand) {
        symbols.push("L".to_string());
    }
    if note.types.contains(&NoteType::Mute) || note.types.contains(&NoteType::PalmMute) {
        symbols.push("x".to_string());
    }
    if note.types.contains(&NoteType::Tap) {
        symbols.push("T".to_string());
    }
    if note.types.contains(&NoteType::Harmonic) {
        symbols.push("H".to_string());
    }
    if note.types.contains(&NoteType::PinchHarmonic) {
        symbols.push("o".to_string());
    }
    if note.types.contains(&NoteType::FretHandMute) {
        symbols.push(".".to_string());
    }
    symbols
}
