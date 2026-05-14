use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SongFileList {
    #[serde(rename = "Data", default)]
    pub data: Vec<SongFile>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SongFile {
    #[serde(rename = "FolderName", default)]
    pub folder_name: String,
    #[serde(rename = "SongName", default)]
    pub song_name: String,
    #[serde(rename = "Artist", default)]
    pub artist: String,
    #[serde(rename = "Album", default)]
    pub album: String,
    #[serde(rename = "Year")]
    pub year: Option<i32>,
    #[serde(rename = "Length", default)]
    pub length: f64,
    #[serde(rename = "Instruments", default)]
    pub instruments: Vec<SongFileInstrument>,
    #[serde(rename = "Lyrics")]
    pub lyrics: Option<SongFileLyrics>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SongFileInstrument {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "IsMain", default)]
    pub is_main: bool,
    #[serde(rename = "Tuning", default)]
    pub tuning: Vec<i16>,
    #[serde(rename = "NoteCount", default)]
    pub note_count: usize,
    #[serde(rename = "CapoFret", default)]
    pub capo_fret: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SongFileLyrics {
    #[serde(rename = "WordCount", default)]
    pub word_count: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SongData {
    #[serde(rename = "Metadata", default)]
    pub metadata: SongMetadata,
    #[serde(rename = "Instruments", default)]
    pub instruments: Vec<SongInstrument>,
    #[serde(rename = "Lyrics")]
    pub lyrics: Option<SongLyrics>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SongMetadata {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Artist", default)]
    pub artist: String,
    #[serde(rename = "Album", default)]
    pub album: String,
    #[serde(rename = "Year")]
    pub year: Option<i32>,
    #[serde(rename = "SongLength", default)]
    pub song_length: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SongInstrument {
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Config", default)]
    pub config: InstrumentConfig,
    #[serde(rename = "Notes", default)]
    pub notes: Vec<NoteBlock>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct InstrumentConfig {
    #[serde(rename = "NoteSpeed", default = "default_note_speed")]
    pub note_speed: f64,
    #[serde(rename = "Tuning", default)]
    pub tuning: Vec<i16>,
    #[serde(rename = "CapoFret", default)]
    pub capo_fret: f32,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct NoteBlock {
    #[serde(rename = "Label", default)]
    pub label: Option<String>,
    #[serde(rename = "Time", default)]
    pub time: f64,
    #[serde(rename = "fws", default)]
    pub fret_window_start: i32,
    #[serde(rename = "fwl", default)]
    pub fret_window_length: i32,
    #[serde(rename = "ChordFlags", default)]
    pub chord_flags: Vec<NoteBlockFlags>,
    #[serde(rename = "Notes", default)]
    pub notes: Vec<SingleNote>,
}

impl NoteBlock {
    pub fn is_chord(&self) -> bool {
        self.notes.len() > 1
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum NoteBlockFlags {
    UNDEFINED,
    MUTE,
}

impl Default for NoteBlockFlags {
    fn default() -> Self {
        Self::UNDEFINED
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SingleNote {
    #[serde(rename = "StringNum", default)]
    pub string_num: i32,
    #[serde(rename = "FretNum", default)]
    pub fret_num: i32,
    #[serde(rename = "Length", default)]
    pub length: f64,
    #[serde(rename = "Type", default)]
    pub note_type: Vec<NoteType>,
    #[serde(rename = "Bends", default)]
    pub bends: Vec<SingleBend>,
    #[serde(rename = "Slide", default)]
    pub slide: Option<SingleSlide>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum NoteType {
    UNDEFINED,
    MISSING,
    CHORD,
    OPEN,
    FRETHANDMUTE,
    TREMOLO,
    HARMONIC,
    PALMMUTE,
    SLAP,
    PLUCK,
    POP,
    HAMMERON,
    PULLOFF,
    SLIDE,
    BEND,
    SUSTAIN,
    TAP,
    PINCHHARMONIC,
    VIBRATO,
    MUTE,
    IGNORE,
    LEFTHAND,
    RIGHTHAND,
    HIGHDENSITY,
    SLIDEUNPITCHEDTO,
    SINGLE,
    CHORDNOTES,
    DOUBLESTOP,
    ACCENT,
    PARENT,
    CHILD,
    ARPEGGIO,
    MISSING2,
    STRUM,
}

impl Default for NoteType {
    fn default() -> Self {
        Self::UNDEFINED
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SingleBend {
    #[serde(rename = "Step", default)]
    pub step: f32,
    #[serde(rename = "Time", default)]
    pub time: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SingleSlide {
    #[serde(rename = "ToFret", default)]
    pub to_fret: i32,
    #[serde(rename = "SlideUnpitched", default)]
    pub slide_unpitched: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SongLyrics {
    #[serde(rename = "Lines", default)]
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LyricLine {
    #[serde(skip)]
    pub start_time: f64,
    #[serde(skip)]
    pub end_time: f64,
    #[serde(rename = "Words", default)]
    pub words: Vec<LyricWord>,
}

impl LyricLine {
    pub fn refresh_timing(&mut self) {
        if let Some(first) = self.words.first() {
            self.start_time = first.time;
        }
        if let Some(last) = self.words.last() {
            self.end_time = last.time + last.length;
        }
    }

    pub fn to_line_text(&self) -> String {
        let mut out = String::new();
        for word in &self.words {
            out.push_str(&word.text);
            if !word.text.ends_with('-') {
                out.push(' ');
            }
        }
        out
    }

    pub fn parts_at(&self, song_position: f64) -> (String, String) {
        let mut first = String::new();
        let mut second = String::new();
        for word in &self.words {
            if word.time <= song_position {
                first.push_str(&word.text);
                if !word.text.ends_with('-') {
                    first.push(' ');
                }
            } else {
                second.push_str(&word.text);
                if !word.text.ends_with('-') {
                    second.push(' ');
                }
            }
        }
        (first, second)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct LyricWord {
    #[serde(rename = "Text", default)]
    pub text: String,
    #[serde(rename = "Time", default)]
    pub time: f64,
    #[serde(rename = "Length", default)]
    pub length: f64,
}

const fn default_note_speed() -> f64 {
    40.0
}
