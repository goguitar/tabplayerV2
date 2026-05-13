use crate::models::{note_symbols, InstrumentConfig, NoteBlock, NoteBlockFlags, NoteType, SingleNote};
use godot::classes::{
    AudioServer, BoxMesh, Control, Label3D, Material, Mesh, MeshInstance3D, Os, PlaneMesh,
    SceneTree, StandardMaterial3D,
};
use godot::classes::label_3d;
use godot::classes::tween;
use godot::builtin::Color;
use godot::prelude::*;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::f64;
use std::path::PathBuf;
use std::sync::OnceLock;

pub struct TweenHelper {
    scene_tree: Gd<SceneTree>,
    control: Gd<Control>,
    prop_name: String,
    initial_prop: Variant,
    final_prop: Variant,
    pub speed: f64,
    pub transition: tween::TransitionType,
}

impl TweenHelper {
    pub fn new(
        scene_tree: Gd<SceneTree>,
        control: Gd<Control>,
        prop_name: &str,
        initial: Variant,
        final_value: Variant,
    ) -> Self {
        Self {
            scene_tree,
            control,
            prop_name: prop_name.to_string(),
            initial_prop: initial,
            final_prop: final_value,
            speed: 1.0,
            transition: tween::TransitionType::QUAD,
        }
    }

    pub fn to_final(&self) {
        let mut scene_tree = self.scene_tree.clone();
        if let Some(mut tween) = scene_tree.create_tween() {
            if let Some(mut tweener) = tween.tween_property(
                &self.control,
                self.prop_name.as_str(),
                &self.final_prop,
                self.speed,
            ) {
                tweener.set_trans(self.transition);
            }
        }
    }

    pub fn to_initial(&self) {
        let mut scene_tree = self.scene_tree.clone();
        if let Some(mut tween) = scene_tree.create_tween() {
            if let Some(mut tweener) = tween.tween_property(
                &self.control,
                self.prop_name.as_str(),
                &self.initial_prop,
                self.speed,
            ) {
                tweener.set_trans(self.transition);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub string_colours: Vec<Color>,
    pub low_string_is_low: bool,
    pub camera_aim_speed: f32,
    pub audio_position_offset_ms: f64,
}

impl Default for Settings {
    fn default() -> Self {
        default_settings()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SettingsFile {
    string_colours: Vec<[f32; 4]>,
    low_string_is_low: bool,
    camera_aim_speed: f32,
    audio_position_offset_ms: f64,
}

static SETTINGS: OnceLock<Mutex<Option<Settings>>> = OnceLock::new();

pub struct SettingsService;

impl SettingsService {
    pub fn settings() -> Settings {
        let store = SETTINGS.get_or_init(|| Mutex::new(None));
        let mut guard = store.lock();
        if let Some(settings) = guard.clone() {
            return settings;
        }

        let settings = load_settings_file().unwrap_or_else(default_settings);
        *guard = Some(settings.clone());
        settings
    }

    pub fn update_settings(settings: Settings) {
        let _ = write_settings_file(&settings);
        let store = SETTINGS.get_or_init(|| Mutex::new(None));
        let mut guard = store.lock();
        *guard = Some(settings);
    }

    pub fn reload_settings() {
        let store = SETTINGS.get_or_init(|| Mutex::new(None));
        let mut guard = store.lock();
        *guard = None;
    }

    pub fn get_color_from_string_num(num: usize) -> Color {
        let settings = Self::settings();
        settings
            .string_colours
            .get(num)
            .cloned()
            .unwrap_or(Color::from_rgb(1.0, 1.0, 1.0))
    }
}

fn settings_path() -> PathBuf {
    let base = Os::singleton().get_user_data_dir().to_string();
    PathBuf::from(base).join("settings.json")
}

fn default_settings() -> Settings {
    Settings {
        string_colours: vec![
            Color::from_rgb(1.0, 0.0, 0.0),
            Color::from_rgb(1.0, 1.0, 0.0),
            Color::from_rgb(0.0, 0.0, 1.0),
            Color::from_rgb(1.0, 0.5, 0.0),
            Color::from_rgb(0.0, 1.0, 0.0),
            Color::from_rgb(0.5, 0.0, 0.5),
        ],
        low_string_is_low: true,
        camera_aim_speed: 10.0,
        audio_position_offset_ms: -50.0,
    }
}

fn load_settings_file() -> Option<Settings> {
    let path = settings_path();
    let data = std::fs::read_to_string(path).ok()?;
    let settings: SettingsFile = serde_json::from_str(&data).ok()?;
    Some(Settings {
        string_colours: settings
            .string_colours
            .iter()
            .map(|c| Color::from_rgba(c[0], c[1], c[2], c[3]))
            .collect(),
        low_string_is_low: settings.low_string_is_low,
        camera_aim_speed: settings.camera_aim_speed,
        audio_position_offset_ms: settings.audio_position_offset_ms,
    })
}

fn write_settings_file(settings: &Settings) -> std::io::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file_data = SettingsFile {
        string_colours: settings
            .string_colours
            .iter()
            .map(|c| [c.r, c.g, c.b, c.a])
            .collect(),
        low_string_is_low: settings.low_string_is_low,
        camera_aim_speed: settings.camera_aim_speed,
        audio_position_offset_ms: settings.audio_position_offset_ms,
    };
    std::fs::write(path, serde_json::to_string_pretty(&file_data).unwrap_or_default())
}

pub fn fixed_width_string(value: &str, length: usize) -> String {
    if length == 0 {
        return String::new();
    }
    if value.len() > length {
        return value[..length].to_string();
    }
    format!("{value:width$}", width = length)
}

pub fn to_min_sec(value: f64, frac: bool) -> String {
    let frac_str = if frac {
        format!(" {:03}ms", ((value % 1.0) * 1000.0) as i32)
    } else {
        String::new()
    };
    format!("{}m {:02}s{}", (value / 60.0).floor(), (value % 60.0).floor(), frac_str)
}

pub fn to_fixed_places(value: f64, count: i32, lead_char: bool) -> String {
    let count = count.max(0) as usize;
    let zeros = "0".repeat(count);
    if lead_char {
        format!("{value:+0.prec$}", prec = count)
    } else {
        format!("{value:0.prec$}", prec = count)
    }
}

pub struct DisplayConst;

impl DisplayConst {
    pub const STRING_LABELS: [char; 6] = ['e', 'A', 'D', 'G', 'B', 'E'];
    pub const TRACK_BOTTOM_WORLD: f32 = -0.5;
    pub const STRING_DISTANCE_APART: f32 = 1.0;

    pub fn calc_note_height_y(string_num: i32) -> f32 {
        let mut string_num = string_num;
        if !SettingsService::settings().low_string_is_low {
            string_num = 5 - string_num;
        }
        string_num as f32 * Self::STRING_DISTANCE_APART
    }

    pub fn calc_fret_pos_z(fret: i32) -> f32 {
        fret as f32 * 1.5
    }

    pub fn calc_in_fret_pos_z(fret: i32) -> f32 {
        Self::calc_fret_pos_z(fret - 1)
            + (Self::calc_fret_pos_z(fret) - Self::calc_fret_pos_z(fret - 1)) / 2.0
    }

    pub fn calc_fret_width_z(fret: i32, width: i32) -> f32 {
        Self::calc_fret_pos_z(fret + width - 1) - Self::calc_fret_pos_z(fret - 1)
    }

    pub fn calc_middle_window_z(fret_start: i32, window_length: i32) -> f32 {
        Self::calc_fret_pos_z(fret_start - 1)
            + Self::calc_fret_width_z(fret_start, window_length) / 2.0
    }
}

pub struct MeshGenerator;

impl MeshGenerator {
    pub fn box_line(color: Color, start: Vector3, end: Vector3) -> Gd<MeshInstance3D> {
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_albedo(color);
        let length = (end - start).length();
        let mut mesh = BoxMesh::new_gd();
        mesh.set_size(Vector3::new(length, 0.1, 0.1));
        mesh.set_material(Some(&mat.upcast::<Material>()));
        let mut mesh_obj = MeshInstance3D::new_alloc();
        mesh_obj.set_transform(Transform3D::new(Basis::IDENTITY, start.lerp(end, 0.5)));
        mesh_obj.set_mesh(Some(&mesh.upcast::<Mesh>()));
        mesh_obj
    }

    pub fn text_vertical(text: &str, pos: Vector3) -> Gd<Label3D> {
        let mut label = Label3D::new_alloc();
        label.set_text(text);
        label.set_font_size(200);
        label.set_draw_flag(label_3d::DrawFlags::SHADED, true);
        label.set_transform(Transform3D::new(
            Basis::from_rows(
                Vector3::new(0.0, 0.0, -1.0),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ),
            pos,
        ));
        label
    }

    pub fn box_shape(color: Color, pos: Vector3) -> Gd<MeshInstance3D> {
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_albedo(color);
        let mut mesh = BoxMesh::new_gd();
        mesh.set_size(Vector3::new(1.0, 1.0, 1.0));
        mesh.set_material(Some(&mat.upcast::<Material>()));
        let mut mesh_obj = MeshInstance3D::new_alloc();
        mesh_obj.set_transform(Transform3D::new(Basis::IDENTITY, pos));
        mesh_obj.set_mesh(Some(&mesh.upcast::<Mesh>()));
        mesh_obj
    }

    pub fn plane(color: Color, center: Vector3, size: Vector2) -> Gd<Node3D> {
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_albedo(color);
        let mut mesh = PlaneMesh::new_gd();
        mesh.set_size(size);
        mesh.set_material(Some(&mat.upcast::<Material>()));
        let mut mesh_obj = MeshInstance3D::new_alloc();
        mesh_obj.set_transform(Transform3D::new(Basis::IDENTITY, center));
        mesh_obj.set_mesh(Some(&mesh.upcast::<Mesh>()));
        mesh_obj.upcast()
    }
}

pub struct NoteGenerator;

impl NoteGenerator {
    pub fn get_basic_note(
        note: &SingleNote,
        config: InstrumentConfig,
        time: f32,
        fret_window_start: i32,
        fret_window_length: i32,
    ) -> Gd<Node3D> {
        let note_pos_z = DisplayConst::calc_in_fret_pos_z(note.fret_num);
        let note_pos = Vector3::new(
            time * config.note_speed,
            DisplayConst::calc_note_height_y(note.string_num),
            note_pos_z,
        );
        let colour = if note.string_num != 255 {
            SettingsService::get_color_from_string_num(note.string_num as usize)
        } else {
            Color::from_rgb(1.0, 0.0, 0.5)
        };
        if note.fret_num == 0 {
            let line_start_z = DisplayConst::calc_fret_pos_z(fret_window_start - 1);
            let across = Vector3::new(
                0.0,
                0.0,
                DisplayConst::calc_fret_width_z(fret_window_start, fret_window_length),
            );
            let line_start = Vector3::new(
                time * config.note_speed,
                DisplayConst::calc_note_height_y(note.string_num),
                line_start_z,
            );
            return MeshGenerator::box_line(colour, line_start, line_start + across).upcast();
        }
        MeshGenerator::box_shape(colour, note_pos).upcast()
    }

    pub fn get_note(note: &SingleNote, config: InstrumentConfig, note_block: &NoteBlock) -> Vec<Gd<Node3D>> {
        let note_pos_z = DisplayConst::calc_in_fret_pos_z(note.fret_num);
        let mut note_pos = Vector3::new(
            note_block.time * config.note_speed,
            DisplayConst::calc_note_height_y(note.string_num),
            note_pos_z,
        );
        let colour = if note.string_num != 255 {
            SettingsService::get_color_from_string_num(note.string_num as usize)
        } else {
            Color::from_rgb(1.0, 0.0, 0.5)
        };
        let mut output = Vec::new();
        if !note.types.contains(&NoteType::Child) {
            if note.fret_num == 0 {
                let line_start_z = DisplayConst::calc_fret_pos_z(note_block.fret_window_start - 1);
                let across = Vector3::new(
                    0.0,
                    0.0,
                    DisplayConst::calc_fret_width_z(note_block.fret_window_start, note_block.fret_window_length),
                );
                let start = Vector3::new(
                    note_block.time * config.note_speed,
                    DisplayConst::calc_note_height_y(note.string_num),
                    line_start_z,
                );
                output.push(MeshGenerator::box_line(colour, start, start + across).upcast());
            } else {
                output.push(MeshGenerator::box_shape(colour, note_pos).upcast());
            }
        }

        let note_text = note_symbols(note).join("");
        if !note_text.is_empty() {
            if note.fret_num == 0 {
                note_pos = Vector3::new(
                    note_pos.x,
                    note_pos.y,
                    DisplayConst::calc_middle_window_z(note_block.fret_window_start, note_block.fret_window_length),
                );
            }
            let mut label = MeshGenerator::text_vertical(&note_text, note_pos - Vector3::new(0.6, 0.0, 0.0));
            label.set_modulate(Color::from_rgb(0.0, 0.0, 0.0));
            output.push(label.upcast());
        }

        output
    }

    pub fn create_note_line(note_block: &NoteBlock, note: &SingleNote, config: InstrumentConfig) -> Vec<Gd<Node3D>> {
        let mut output = Vec::new();
        let mut note_pos = Vector3::new(
            note_block.time * config.note_speed,
            DisplayConst::calc_note_height_y(note.string_num),
            DisplayConst::calc_in_fret_pos_z(note.fret_num),
        );
        if note.fret_num == 0 {
            note_pos = Vector3::new(
                note_pos.x,
                note_pos.y,
                DisplayConst::calc_middle_window_z(note_block.fret_window_start, note_block.fret_window_length),
            );
        }
        if !note.types.contains(&NoteType::Sustain) {
            return output;
        }
        if note.length == 0.0 {
            return output;
        }

        let note_colour = SettingsService::get_color_from_string_num(note.string_num as usize);
        let mut final_line_pos = note_pos + Vector3::new(config.note_speed * note.length, 0.0, 0.0);
        if note.types.contains(&NoteType::Bend) {
            if let Some(bends) = &note.bends {
                let mut last_pos = note_pos;
                for bend in bends {
                    let end_pos = Vector3::new(
                        bend.time * config.note_speed,
                        note_pos.y + DisplayConst::STRING_DISTANCE_APART * bend.step,
                        note_pos.z,
                    );
                    if last_pos != end_pos {
                        output.push(MeshGenerator::box_line(note_colour, last_pos, end_pos).upcast());
                    }
                    last_pos = end_pos;
                }
                if note_block.time + note.length > bends.last().map(|b| b.time).unwrap_or(note_block.time) {
                    let end_pos = Vector3::new((note_block.time + note.length) * config.note_speed, note_pos.y, note_pos.z);
                    output.push(MeshGenerator::box_line(note_colour, last_pos, end_pos).upcast());
                }
                final_line_pos = last_pos;
            }
        } else if note.types.contains(&NoteType::Slide) || note.types.contains(&NoteType::SlideUnpitchedTo) {
            if let Some(slide) = &note.slide {
                let end_pos = Vector3::new(
                    (note_block.time + note.length) * config.note_speed,
                    note_pos.y,
                    DisplayConst::calc_in_fret_pos_z(slide.to_fret),
                );
                output.push(MeshGenerator::box_line(note_colour, note_pos, end_pos).upcast());
                final_line_pos = end_pos;
            }
        } else if !note.types.contains(&NoteType::Vibrato) {
            output.push(MeshGenerator::box_line(
                note_colour,
                note_pos,
                note_pos + Vector3::new(config.note_speed * note.length, 0.0, 0.0),
            ).upcast());
        } else {
            let mut wibble = 0;
            let mut cur_x = note_pos.x;
            let mut last_pos = note_pos;
            while cur_x < final_line_pos.x {
                cur_x += 1.5;
                let cur_pos = Vector3::new(
                    cur_x,
                    note_pos.y + (wibble as f32 * std::f32::consts::FRAC_PI_2).sin() * 0.4,
                    note_pos.z + (wibble as f32 * std::f32::consts::FRAC_PI_2).cos() * 0.2,
                );
                output.push(MeshGenerator::box_line(note_colour, last_pos, cur_pos).upcast());
                wibble += 1;
                last_pos = cur_pos;
            }
        }

        if note.types.contains(&NoteType::Tremolo) {
            let mut cur_pos = note_pos;
            while cur_pos.x < final_line_pos.x {
                let mut box_note = MeshGenerator::box_shape(note_colour, cur_pos);
                let new_scale = box_note.get_scale() * 0.45;
                box_note.set_scale(new_scale);
                output.push(box_note.upcast());
                let direction = (final_line_pos - note_pos).normalized();
                cur_pos += direction * 1.25;
            }
        }

        output
    }
}

pub fn audio_bus_index() -> i32 {
    AudioServer::singleton().get_bus_index("SongPlayback")
}
