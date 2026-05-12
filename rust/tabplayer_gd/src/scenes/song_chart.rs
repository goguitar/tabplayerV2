use super::shared::*;

#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct SongChart {
    #[base]
    base: Base<Node3D>,
    pub instrument: Option<Instrument>,
    last_chord: Option<NoteBlock>,
}

#[godot_api]
impl INode3D for SongChart {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            instrument: None,
            last_chord: None,
        }
    }

    fn ready(&mut self) {
        if let Some(instrument) = &self.instrument {
            let items = self.load_notes(instrument);
            for item in items {
                self.base_mut().add_child(Some(&item.upcast::<Node>()));
            }
        }
    }
}

#[godot_api]
impl SongChart {
    fn load_notes(&mut self, instrument: &Instrument) -> Vec<Gd<Node3D>> {
        let mut result = Vec::new();
        let mut fret_last: HashMap<i32, f32> = HashMap::new();
        for note_block in &instrument.notes {
            if note_block.is_chord() {
                result.extend(self.chord(note_block, instrument.config));
                self.last_chord = Some(note_block.clone());
            } else {
                result.extend(self.single_note(note_block, instrument.config));
                self.last_chord = None;
            }
            for note in note_block.notes.iter().rev() {
                if note.fret_num == 0 || note.fret_num > 30 {
                    continue;
                }
                let entry = fret_last.entry(note.fret_num).or_insert(-10.0);
                if (note_block.time - *entry).abs() > 0.6 {
                    *entry = note_block.time;
                    let z_pos = DisplayConst::calc_in_fret_pos_z(note.fret_num);
                    result.push(MeshGenerator::text_vertical(
                        &note.fret_num.to_string(),
                        Vector3::new(
                            note_block.time * instrument.config.note_speed,
                            DisplayConst::calc_note_height_y(note.string_num) + 1.0,
                            z_pos,
                        ),
                    ).upcast());
                }
            }
        }
        result.extend(generate_note_block_frets(instrument));
        result
    }

    fn chord(&self, note_block: &NoteBlock, config: InstrumentConfig) -> Vec<Gd<Node3D>> {
        let mut list = Vec::new();
        let line_start_z = DisplayConst::calc_fret_pos_z(note_block.fret_window_start - 1);
        let across = Vector3::new(0.0, 0.0, DisplayConst::calc_fret_width_z(note_block.fret_window_start, note_block.fret_window_length));
        let bottom_left = Vector3::new(note_block.time * config.note_speed, DisplayConst::TRACK_BOTTOM_WORLD + 0.01, line_start_z);
        let chord_dir_up = Vector3::new(0.0, 1.0, 0.0) * 6.0 * DisplayConst::STRING_DISTANCE_APART;
        if note_block.chord_flags.contains(&NoteBlockFlags::Mute) {
            list.push(MeshGenerator::box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left, bottom_left + across + chord_dir_up * 0.5).upcast());
            list.push(MeshGenerator::box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left + across, bottom_left + chord_dir_up * 0.5).upcast());
        } else if !note_block.is_same_chord_as(self.last_chord.as_ref()) {
            for note in &note_block.notes {
                list.extend(NoteGenerator::get_note(note, config, note_block));
                list.extend(NoteGenerator::create_note_line(note_block, note, config));
            }
            if let Some(label) = &note_block.label {
                list.push(MeshGenerator::text_vertical(label, Vector3::new(
                    note_block.time * config.note_speed,
                    7.0 * DisplayConst::STRING_DISTANCE_APART,
                    DisplayConst::calc_in_fret_pos_z(note_block.fret_window_start),
                )).upcast());
            }
        }
        if !note_block.notes.iter().all(|n| n.types.contains(&NoteType::Child)) {
            list.push(MeshGenerator::box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left + chord_dir_up, bottom_left).upcast());
            list.push(MeshGenerator::box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left, bottom_left + across).upcast());
            list.push(MeshGenerator::box_line(Color::from_rgb(0.83, 0.83, 0.83), bottom_left + across, bottom_left + across + chord_dir_up).upcast());
        }
        list
    }

    fn single_note(&self, note_block: &NoteBlock, config: InstrumentConfig) -> Vec<Gd<Node3D>> {
        let mut list = Vec::new();
        if note_block.notes.len() != 1 {
            return list;
        }
        let note = &note_block.notes[0];
        list.extend(NoteGenerator::get_note(note, config, note_block));
        list.extend(NoteGenerator::create_note_line(note_block, note, config));
        if note.fret_num != 0 {
            let note_pos = Vector3::new(
                note_block.time * config.note_speed,
                DisplayConst::calc_note_height_y(note.string_num),
                DisplayConst::calc_in_fret_pos_z(note.fret_num),
            );
            let dir = Vector3::new(0.0, note_pos.y - DisplayConst::TRACK_BOTTOM_WORLD, 0.0);
            let start = Vector3::new(note_pos.x, DisplayConst::TRACK_BOTTOM_WORLD, note_pos.z);
            list.push(MeshGenerator::box_line(SettingsService::get_color_from_string_num(note.string_num as usize), start, start + dir).upcast());
            let pos = Vector3::new(
                note_pos.x,
                DisplayConst::TRACK_BOTTOM_WORLD + 0.01,
                DisplayConst::calc_fret_pos_z(note.fret_num - 1),
            );
            let across = Vector3::new(0.0, 0.0, DisplayConst::calc_fret_width_z(note.fret_num, 1));
            list.push(MeshGenerator::box_line(SettingsService::get_color_from_string_num(note.string_num as usize), pos, pos + across).upcast());
        }
        list
    }
}

fn generate_note_block_frets(instrument: &Instrument) -> Vec<Gd<Node3D>> {
    let mut output = Vec::new();
    let mut start_section = -10.0;
    let mut cur_start = -1;
    let mut cur_length = -1;
    for note in &instrument.notes {
        if cur_start != note.fret_window_start || cur_length != note.fret_window_length {
            if cur_start > 0 {
                while start_section < note.time {
                    let length = (note.time - start_section).min(10.0);
                    output.push(create_window_piece(cur_start, cur_length, start_section + length, start_section, instrument.config));
                    start_section += length;
                }
            }
            cur_start = note.fret_window_start;
            cur_length = note.fret_window_length;
            start_section = note.time;
        }
    }
    if cur_start > 0 {
        if let Some(last_note) = instrument.notes.last() {
            while start_section < last_note.time + 1.0 {
                let length = (last_note.time + 1.0 - start_section).min(10.0);
                output.push(create_window_piece(cur_start, cur_length, start_section + length, start_section, instrument.config));
                start_section += length;
            }
        }
    }
    output
}

fn create_window_piece(fret: i32, length: i32, end_time: f32, start_time: f32, config: InstrumentConfig) -> Gd<Node3D> {
    let across = DisplayConst::calc_fret_width_z(fret, length);
    let pos = Vector3::new(
        (end_time + start_time) / 2.0 * config.note_speed - 0.5,
        DisplayConst::TRACK_BOTTOM_WORLD - 0.01,
        DisplayConst::calc_fret_pos_z(fret - 1) + across / 2.0,
    );
    MeshGenerator::plane(Color::from_rgb(0.28, 0.24, 0.55), pos, Vector2::new(config.note_speed * (end_time - start_time), across))
}
