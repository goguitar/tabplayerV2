use crate::common::*;
use godot::classes::base_material_3d::Transparency;

#[derive(GodotClass)]
#[class(base=Node)]
struct SongScene {
    #[base]
    base: Base<Node>,
    clock: SongClock,
    loop_a: Option<f64>,
    loop_b: Option<f64>,
    song_data: SongData,
    instrument_name: String,
    song_id: String,
    has_audio_stream: bool,
    instrument_menu_names: Vec<String>,
    guitar_chart_root: Option<Gd<Node3D>>,
    guitar_camera: Option<Gd<Camera3D>>,
    song_chart_root: Option<Gd<Node3D>>,
    song_chart_nodes: Vec<Gd<Node3D>>,
    last_note_block_time: Option<f64>,
    last_note_block_node: Option<Gd<Node3D>>,
    last_chord_block: Option<tabplayer_parser::models::NoteBlock>,
    cached_song_position: Option<f64>,
}

#[godot_api]
impl INode for SongScene {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            clock: SongClock::new(48_000),
            loop_a: None,
            loop_b: None,
            song_data: SongData::default(),
            instrument_name: String::new(),
            song_id: String::new(),
            has_audio_stream: false,
            instrument_menu_names: Vec::new(),
            guitar_chart_root: None,
            guitar_camera: None,
            song_chart_root: None,
            song_chart_nodes: Vec::new(),
            last_note_block_time: None,
            last_note_block_node: None,
            last_chord_block: None,
            cached_song_position: None,
        }
    }

    fn ready(&mut self) {
        if let Some(pending) = take_pending_song() {
            self.song_id = pending.song_id.clone();
            self.instrument_name = pending.instrument;
            if let Some(data) = read_song_data(&self.song_id) {
                self.song_data = data;
            }
        } else {
            let songs = catalog_list_song_files();
            if let Some(first_song) = songs.first() {
                self.song_id = first_song.folder_name.clone();
                if let Some(data) = read_song_data(&self.song_id) {
                    self.instrument_name = data
                        .instruments
                        .iter()
                        .find(|x| !is_hidden_instrument_name(&x.name))
                        .map(|x| x.name.clone())
                        .unwrap_or_else(|| "lead".to_string());
                    self.song_data = data;
                }
            }
        }

        self.clock.play();
        self.setup_audio_playback();
        self.setup_guitar_chart();
        self.load_instrument_from_state();
        self.populate_instrument_menu();
        self.update_labels();
    }

    fn process(&mut self, delta: f64) {
        self.cached_song_position = None;
        if self.has_audio_stream {
            let song_time = self.get_song_position();
            self.clock.seek_seconds(song_time);
        } else {
            self.clock.tick(delta);
        }
        let mut song_time = self.clock.song_time_seconds();

        if let (Some(a), Some(b)) = (self.loop_a, self.loop_b) {
            if b > a && a < song_time && delta + song_time > b {
                self.clock.seek_seconds(a);
                song_time = a;
                if self.has_audio_stream {
                    let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
                    player.seek(a as f32);
                }
            }
        }

        if self.song_data.metadata.song_length > 0.0 && song_time > self.song_data.metadata.song_length {
            self.clock.seek_seconds(self.song_data.metadata.song_length);
            self.clock.pause();
            song_time = self.song_data.metadata.song_length;
        }

        self.update_guitar_chart(delta, song_time);

        let mut running = self.base().get_node_as::<Label>("RunningDetailsLabel");
        let running_text = format!("{}fps | {:05.1}ms\n{}", Engine::singleton().get_frames_per_second(), delta * 1000.0, to_min_sec_msec(song_time, true));
        running.set_text(&running_text);

        let mut pos = self
            .base()
            .get_node_as::<LineEdit>("GridContainer/PositionSetLineEdit");
        let pos_text = to_min_sec_msec(song_time, true);
        pos.set_text(&pos_text);

        let mut speed_label = self.base().get_node_as::<Label>("GridContainer/SongSpeedLabel");
        let speed_text = format!("{:.1}%", self.clock.speed * 100.0);
        speed_label.set_text(&speed_text);

        let mut a_label = self.base().get_node_as::<Label>("GridContainer/ABLabelStart");
        let a_text = self.loop_a.map(|x| to_min_sec_msec(x, true)).unwrap_or_default();
        a_label.set_text(&a_text);

        let mut b_label = self.base().get_node_as::<Label>("GridContainer/ABLabelEnd");
        let b_text = self.loop_b.map(|x| to_min_sec_msec(x, true)).unwrap_or_default();
        b_label.set_text(&b_text);

        self.update_details_label(song_time);
        self.update_lyrics(song_time);
    }
}

#[godot_api]
impl SongScene {
    fn current_instrument(&self) -> Option<&tabplayer_parser::models::SongInstrument> {
        self.song_data
            .instruments
            .iter()
            .filter(|x| !is_hidden_instrument_name(&x.name))
            .find(|x| x.name == self.instrument_name)
            .or_else(|| {
                self.song_data
                    .instruments
                    .iter()
                    .filter(|x| !is_hidden_instrument_name(&x.name))
                    .min_by_key(|x| instrument_order_key(&x.name))
            })
    }

    fn setup_guitar_chart(&mut self) {
        if self.guitar_chart_root.is_some() {
            return;
        }

        let mut root = Node3D::new_alloc();
        root.set_name("GuitarChartRoot");
        let root_node: Gd<Node> = root.clone().upcast();
        self.base_mut().add_child(&root_node);

        let mut material = StandardMaterial3D::new_gd();
        material.set_albedo(Color::from_rgb(0.82, 0.71, 0.55));
        let mut plane_mesh = PlaneMesh::new_gd();
        plane_mesh.set_size(Vector2::new(6.0, 6.0));
        plane_mesh.set_center_offset(Vector3::new(2.5, 0.0, -3.0));
        let plane_material: Gd<Material> = material.upcast();
        plane_mesh.set_material(&plane_material);
        let mut plane = MeshInstance3D::new_alloc();
        let plane_mesh_up: Gd<Mesh> = plane_mesh.upcast();
        plane.set_mesh(&plane_mesh_up);
        plane.set_transform(Transform3D::new(
            Basis::from_rows(
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
            ),
            Vector3::ZERO,
        ));
        let plane_node: Gd<Node> = plane.upcast();
        root.add_child(&plane_node);

        let mut camera = Camera3D::new_alloc();
        camera.set_fov(60.0);
        camera.set_transform(Transform3D::new(
            Basis::from_rows(
                Vector3::new(0.0, 0.310809, -0.950472),
                Vector3::new(0.0, 0.950472, 0.310809),
                Vector3::new(1.0, 0.0, 0.0),
            ),
            Vector3::new(-12.0, 10.0, 8.0),
        ));
        let _ = camera.call("make_current", &[]);
        self.guitar_camera = Some(camera.clone());
        let camera_node: Gd<Node> = camera.upcast();
        root.add_child(&camera_node);

        let mut light = DirectionalLight3D::new_alloc();
        light.set_transform(Transform3D::new(
            Basis::from_rows(
                Vector3::new(-0.177838, 0.752991, -0.633544),
                Vector3::new(-0.317607, 0.565433, 0.761191),
                Vector3::new(0.931397, 0.336587, 0.1386),
            ),
            Vector3::ZERO,
        ));
        let light_node: Gd<Node> = light.upcast();
        root.add_child(&light_node);

        for string in 0..6 {
            let mut str_mesh = MeshInstance3D::new_alloc();
            let mut box_mesh = BoxMesh::new_gd();
            box_mesh.set_size(Vector3::new(0.08, 0.08, 50.0));
            let mut mat = StandardMaterial3D::new_gd();
            mat.set_albedo(string_color(string));
            let line_material: Gd<Material> = mat.upcast();
            box_mesh.set_material(&line_material);
            let string_mesh: Gd<Mesh> = box_mesh.upcast();
            str_mesh.set_mesh(&string_mesh);
            str_mesh.set_position(Vector3::new(0.0, calc_note_height_y(string), 25.0));
            let string_node: Gd<Node> = str_mesh.upcast();
            root.add_child(&string_node);
        }

        for fret in 0..25 {
            let mut fret_mesh = MeshInstance3D::new_alloc();
            let mut box_mesh = BoxMesh::new_gd();
            box_mesh.set_size(Vector3::new(0.03, 5.0 + calc_track_bottom_world().abs() * 2.0, 0.03));
            let mut mat = StandardMaterial3D::new_gd();
            mat.set_albedo(Color::from_rgb(0.82, 0.71, 0.55));
            let fret_material: Gd<Material> = mat.upcast();
            box_mesh.set_material(&fret_material);
            let fret_mesh_res: Gd<Mesh> = box_mesh.upcast();
            fret_mesh.set_mesh(&fret_mesh_res);
            fret_mesh.set_position(Vector3::new(0.0, 2.5, calc_fret_pos_z(fret)));
            let fret_node: Gd<Node> = fret_mesh.upcast();
            root.add_child(&fret_node);

            let mut path_mesh = MeshInstance3D::new_alloc();
            let mut path_box = BoxMesh::new_gd();
            path_box.set_size(Vector3::new(0.03, 6.0, 0.03));
            let mut path_mat = StandardMaterial3D::new_gd();
            path_mat.set_albedo(Color::from_rgb(0.33, 0.33, 0.33));
            let path_material: Gd<Material> = path_mat.upcast();
            path_box.set_material(&path_material);
            let path_mesh_res: Gd<Mesh> = path_box.upcast();
            path_mesh.set_mesh(&path_mesh_res);
            path_mesh.set_transform(Transform3D::new(
                Basis::from_rows(
                    Vector3::new(0.0, 5.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 1.0),
                ),
                Vector3::new(15.0, calc_track_bottom_world(), calc_fret_pos_z(fret)),
            ));
            let path_node: Gd<Node> = path_mesh.upcast();
            root.add_child(&path_node);
        }

        for fret_label in [3_i32, 5, 7, 9, 12, 15, 17, 19, 21, 24] {
            let mut label = Label3D::new_alloc();
            label.set_text(&fret_label.to_string());
            label.set_font_size(200);
            label.set_transform(Transform3D::new(
                Basis::from_rows(
                    Vector3::new(0.0, 1.0, 0.0),
                    Vector3::new(0.0, 0.0, 1.0),
                    Vector3::new(1.0, 0.0, 0.0),
                ),
                Vector3::new(-0.25, calc_track_bottom_world(), calc_in_fret_pos_z(fret_label)),
            ));
            root.add_child(&label.clone().upcast::<Node>());
        }

        self.guitar_chart_root = Some(root);
    }

    fn load_instrument_from_state(&mut self) {
        if let Some(mut last) = self.last_note_block_node.take() {
            last.queue_free();
        }
        self.last_note_block_time = None;
        self.rebuild_song_chart();
    }

    fn clear_song_chart(&mut self) {
        for mut node in self.song_chart_nodes.drain(..) {
            node.queue_free();
        }
        if let Some(mut node) = self.song_chart_root.take() {
            node.queue_free();
        }
    }

    fn rebuild_song_chart(&mut self) {
        self.clear_song_chart();
        self.last_chord_block = None;

        let Some(instrument) = self.current_instrument().cloned() else {
            return;
        };

        let mut chart_root = Node3D::new_alloc();
        chart_root.set_name("SongChartRoot");

        for block in &instrument.notes {
            if block.is_chord() {
                self.add_chord_block(&mut chart_root, &instrument, block);
                self.last_chord_block = Some(block.clone());
            } else {
                self.add_single_note_block(&mut chart_root, &instrument, block);
                self.last_chord_block = None;
            }
        }

        for node in generate_note_block_frets(&instrument) {
            chart_root.add_child(&node.clone().upcast::<Node>());
            self.song_chart_nodes.push(node);
        }

        let chart_node: Gd<Node> = chart_root.clone().upcast();
        self.base_mut().add_child(&chart_node);
        self.song_chart_root = Some(chart_root);
    }

    fn add_single_note_block(&mut self, chart_root: &mut Gd<Node3D>, instrument: &tabplayer_parser::models::SongInstrument, block: &tabplayer_parser::models::NoteBlock) {
        if block.notes.is_empty() {
            return;
        }
        let note = &block.notes[0];
        for node in make_note_geometry(note, &instrument.config, block) {
            chart_root.add_child(&node.clone().upcast::<Node>());
            self.song_chart_nodes.push(node);
        }

        if note.fret_num != 0 {
            let note_pos = Vector3::new(
                block.time as f32 * instrument.config.note_speed as f32,
                calc_note_height_y(note.string_num),
                calc_in_fret_pos_z(note.fret_num),
            );
            let start = Vector3::new(note_pos.x, calc_track_bottom_world(), note_pos.z);
            let vertical = box_line(string_color(note.string_num), start, Vector3::new(note_pos.x, note_pos.y, note_pos.z));
            chart_root.add_child(&vertical.clone().upcast::<Node>());
            self.song_chart_nodes.push(vertical);

            let pos = Vector3::new(note_pos.x, calc_track_bottom_world() + 0.01, calc_fret_pos_z(note.fret_num - 1));
            let horizontal = box_line(
                string_color(note.string_num),
                pos,
                pos + Vector3::new(0.0, 0.0, calc_fret_width_z(note.fret_num, 1)),
            );
            chart_root.add_child(&horizontal.clone().upcast::<Node>());
            self.song_chart_nodes.push(horizontal);
        }
    }

    fn add_chord_block(&mut self, chart_root: &mut Gd<Node3D>, instrument: &tabplayer_parser::models::SongInstrument, block: &tabplayer_parser::models::NoteBlock) {
        let is_same_chord = self
            .last_chord_block
            .as_ref()
            .is_some_and(|last| is_same_chord_as(block, last) && (block.time - last.time).abs() <= 1.2);

        let is_mute = block
            .chord_flags
            .iter()
            .any(|x| *x == tabplayer_parser::models::NoteBlockFlags::MUTE);

        if is_mute {
            let line_start_z = calc_fret_pos_z(block.fret_window_start - 1);
            let across = calc_fret_width_z(block.fret_window_start, block.fret_window_length);
            let bottom_left = Vector3::new(
                block.time as f32 * instrument.config.note_speed as f32,
                calc_track_bottom_world() + 0.01,
                line_start_z,
            );
            let chord_dir = Vector3::new(0.0, 6.0 * calc_string_distance() * 0.5, 0.0);
            for node in [
                box_line_translucent(
                    Color::from_rgba(0.83, 0.83, 0.83, 0.52),
                    bottom_left,
                    bottom_left + Vector3::new(0.0, 0.0, across) + chord_dir,
                ),
                box_line_translucent(
                    Color::from_rgba(0.83, 0.83, 0.83, 0.52),
                    bottom_left + Vector3::new(0.0, 0.0, across),
                    bottom_left + chord_dir,
                ),
            ] {
                chart_root.add_child(&node.clone().upcast::<Node>());
                self.song_chart_nodes.push(node);
            }
        } else if !is_same_chord {
            for note in &block.notes {
                for node in make_note_geometry(note, &instrument.config, block) {
                    chart_root.add_child(&node.clone().upcast::<Node>());
                    self.song_chart_nodes.push(node);
                }
            }

            if let Some(label) = &block.label {
                if !label.trim().is_empty() {
                    let node = text_vertical(
                        label,
                        Vector3::new(
                            block.time as f32 * instrument.config.note_speed as f32,
                            7.0 * calc_string_distance(),
                            calc_in_fret_pos_z(block.fret_window_start),
                        ),
                    );
                    chart_root.add_child(&node.clone().upcast::<Node>());
                    self.song_chart_nodes.push(node);
                }
            }
        }

        let line_start_z = calc_fret_pos_z(block.fret_window_start - 1);
        let across = calc_fret_width_z(block.fret_window_start, block.fret_window_length);
        let bottom_left = Vector3::new(
            block.time as f32 * instrument.config.note_speed as f32,
            calc_track_bottom_world() + 0.01,
            line_start_z,
        );
        let chord_dir = Vector3::new(0.0, 6.0 * calc_string_distance(), 0.0);
        for node in [
            box_line_translucent(
                Color::from_rgba(0.83, 0.83, 0.83, 0.52),
                bottom_left + chord_dir,
                bottom_left,
            ),
            box_line_translucent(
                Color::from_rgba(0.83, 0.83, 0.83, 0.52),
                bottom_left,
                bottom_left + Vector3::new(0.0, 0.0, across),
            ),
            box_line_translucent(
                Color::from_rgba(0.83, 0.83, 0.83, 0.52),
                bottom_left + Vector3::new(0.0, 0.0, across),
                bottom_left + Vector3::new(0.0, 0.0, across) + chord_dir,
            ),
        ] {
            chart_root.add_child(&node.clone().upcast::<Node>());
            self.song_chart_nodes.push(node);
        }
    }

    fn update_guitar_chart(&mut self, delta: f64, song_time: f64) {
        if let Some(next) = self.next_note_block() {
            if let Some(mut cam) = self.guitar_camera.clone() {
                let cam_move_speed = 10.0_f32 / 50.0;
                let want_pos = calc_middle_window_z(next.fret_window_start, next.fret_window_length);
                let cur_pos = cam.get_position();
                let new_z = cur_pos.z * (1.0 - delta as f32 * cam_move_speed) + want_pos * delta as f32 * cam_move_speed;
                cam.set_position(Vector3::new(cur_pos.x, cur_pos.y, new_z));
            }
        }

        if let Some(mut root) = self.guitar_chart_root.clone() {
            if let Some(instrument) = self.current_instrument() {
                let root_pos = root.get_position();
                root.set_position(Vector3::new(
                    song_time as f32 * instrument.config.note_speed as f32,
                    root_pos.y,
                    root_pos.z,
                ));
            }

            let next_time = self.next_note_block().map(|x| x.time);
            if next_time != self.last_note_block_time {
                self.last_note_block_time = next_time;
                if let Some(mut n) = self.last_note_block_node.take() {
                    n.queue_free();
                }

                if let (Some(block), Some(instrument)) = (
                    self.next_note_block().cloned(),
                    self.current_instrument().cloned(),
                ) {
                    let mut node = Node3D::new_alloc();
                    for note in &block.notes {
                        let basic = basic_note(
                            note,
                            &instrument.config,
                            0.2_f32 / instrument.config.note_speed as f32,
                            block.fret_window_start,
                            block.fret_window_length,
                        );
                        node.add_child(&basic.clone().upcast::<Node>());
                    }
                    root.add_child(&node.clone().upcast::<Node>());
                    self.last_note_block_node = Some(node);
                }
            }
        }
    }

    fn setup_audio_playback(&mut self) {
        self.has_audio_stream = false;
        let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");

        let audio_stream = match load_song_audio_stream(&self.song_id) {
            Ok(stream) => stream,
            Err(err) => {
                godot_error!("[audio] failed loading stream for '{}': {}", self.song_id, err);
            let mut details = self
                .base()
                .get_node_as::<Label>("DetailsVBoxContainer/SongDetailsLabel");
            let details_text = format!(
                "Length: {}\nInstrument: {}\nAudio: missing ({})",
                to_min_sec(self.song_data.metadata.song_length),
                self.instrument_name,
                err,
            );
            details.set_text(&details_text);
            return;
            }
        };

        let _ = player.call("set_stream", &[audio_stream.to_variant()]);
        player.play();
        self.has_audio_stream = true;
    }

    fn populate_instrument_menu(&mut self) {
        let mut menu_button = self
            .base()
            .get_node_as::<MenuButton>("DetailsVBoxContainer/VBoxContainer/InstrumentMenuButton");
        menu_button.set_text(&self.instrument_name);

        let mut names = self
            .song_data
            .instruments
            .iter()
            .filter(|x| !is_hidden_instrument_name(&x.name))
            .map(|x| x.name.clone())
            .collect::<Vec<_>>();
        names.sort_by_key(|x| instrument_order_key(x));
        self.instrument_menu_names = names;

        if let Some(mut popup) = menu_button.get_popup() {
            popup.clear();
            for instrument in &self.instrument_menu_names {
                popup.add_item(instrument);
            }
            if let Some(index) = self
                .instrument_menu_names
                .iter()
                .position(|x| x.eq_ignore_ascii_case(&self.instrument_name))
            {
                popup.set_item_checked(index as i32, true);
            }
            let callable = self.base().callable("InstrumentChanged");
            let _ = popup.connect("id_pressed", &callable);
        }
    }

    fn update_labels(&mut self) {
        let year = self
            .song_data
            .metadata
            .year
            .map(|x| x.to_string())
            .unwrap_or_else(|| "?".to_string());

        let mut info = self
            .base()
            .get_node_as::<Label>("DetailsVBoxContainer/SongInfoLabel");
        let info_text = format!(
            "{} ({})\n{}\n{}",
            self.song_data.metadata.name, year, self.song_data.metadata.artist, self.song_data.metadata.album
        );
        info.set_text(&info_text);

        self.update_details_label(self.get_song_position());
        self.update_lyrics(self.get_song_position());
    }

    fn update_details_label(&mut self, song_position: f64) {
        let mut details = self
            .base()
            .get_node_as::<Label>("DetailsVBoxContainer/SongDetailsLabel");

        let Some(instrument) = self.current_instrument() else {
            details.set_text("No instrument");
            return;
        };

        let next_text = self
            .next_note_block()
            .map(|next| {
                let mut next_note = self.base().get_node_as::<Label>("GridContainer/SkipToNextLabel2");
                next_note.set_text(&format!("at {}", to_min_sec_msec(next.time, false)));
                format!(
                    "Next Note: {} in {:.1}",
                    to_min_sec_msec(next.time, false),
                    next.time - song_position
                )
            })
            .unwrap_or_else(|| "No note".to_string());

        let note_count = instrument.notes.iter().filter(|x| !x.is_chord()).count();
        let chord_count = instrument.notes.iter().filter(|x| x.is_chord()).count();
        let first = instrument.notes.first().map(|x| to_min_sec_msec(x.time, false)).unwrap_or_default();
        let last = instrument.notes.last().map(|x| to_min_sec_msec(x.time, false)).unwrap_or_default();

        details.set_text(&format!(
            "---------\nTuning: {}\nNotes: {}\nChords: {}\nFirst note @ {}\nLast note @ {}\n---------\n{}\n",
            calc_tuning_name(&instrument.config.tuning),
            note_count,
            chord_count,
            first,
            last,
            next_text,
        ));
    }

    fn update_lyrics(&mut self, song_position: f64) {
        let mut label = self.base().get_node_as::<RichTextLabel>("HBoxContainer/LyricsLabel");
        label.clear();
        label.push_font_size(40);

        let Some(lyrics) = &self.song_data.lyrics else {
            label.set_text("");
            return;
        };

        let mut idx = None;
        for (i, line) in lyrics.lines.iter().enumerate() {
            if line.end_time >= song_position {
                idx = Some(i);
                break;
            }
        }
        if idx.is_none() {
            if let Some(first) = lyrics.lines.first() {
                if first.start_time > song_position {
                    idx = Some(0);
                }
            }
        }
        let Some(i) = idx else {
            label.set_text("");
            return;
        };

        let cur = &lyrics.lines[i];
        let (part_a, part_b) = cur.parts_at(song_position);
        label.push_color(Color::from_rgb(1.0, 0.0, 0.0));
        label.add_text(&part_a);
        label.push_color(Color::from_rgb(1.0, 1.0, 0.0));
        label.add_text(&part_b);

        if let Some(next) = lyrics.lines.get(i + 1) {
            label.add_text(&format!("\n{}", next.to_line_text()));
        }
    }

    fn next_note_block(&self) -> Option<&tabplayer_parser::models::NoteBlock> {
        let instrument = self.current_instrument()?;
        let song_pos = self.get_song_position();
        instrument.notes.iter().find(|x| x.time > song_pos)
    }

    fn get_song_position(&self) -> f64 {
        if self.has_audio_stream {
            let player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            let mut time = player.get_playback_position() as f64;
            time += AudioServer::singleton().get_time_since_last_mix() as f64;
            time -= AudioServer::singleton().get_output_latency() as f64;
            return time.max(0.0);
        }
        self.clock.song_time_seconds()
    }

    #[func]
    fn _init(&mut self, _state: Variant) {}

    #[func]
    fn InstrumentChanged(&mut self, id: i64) {
        if let Some(instrument) = self.instrument_menu_names.get(id as usize) {
            self.instrument_name = instrument.clone();
            self.update_labels();
            self.load_instrument_from_state();
        }
    }

    #[func]
    fn SongFinished(&mut self) {
        self.clock.seek_seconds(0.0);
        self.clock.pause();
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.play();
            player.seek(0.0);
            player.set_stream_paused(true);
        }
    }

    #[func]
    fn PauseButton_Pressed(&mut self) {
        if self.clock.playing {
            self.clock.pause();
            if self.has_audio_stream {
                let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
                player.set_stream_paused(true);
            }
        } else {
            self.clock.play();
            if self.has_audio_stream {
                let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
                player.set_stream_paused(false);
            }
        }
    }

    #[func]
    fn Pause(&mut self) {
        self.clock.pause();
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.set_stream_paused(true);
        }
    }

    #[func]
    fn Resume(&mut self) {
        self.clock.play();
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.set_stream_paused(false);
        }
    }

    #[func]
    fn Quit(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/SongPick.tscn");
    }

    #[func]
    fn Skip10Sec(&mut self) {
        let target = self.clock.song_time_seconds() + 10.0;
        self.clock.seek_seconds(target);
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.seek(target as f32);
        }
    }

    #[func]
    fn Back10Sec(&mut self) {
        let target = (self.clock.song_time_seconds() - 10.0).max(0.0);
        self.clock.seek_seconds(target);
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.seek(target as f32);
        }
    }

    #[func]
    fn SkipToNext(&mut self) {
        let Some(next) = self.next_note_block() else {
            return;
        };
        let target = (next.time - 1.5).max(0.0);
        self.clock.seek_seconds(target);
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.seek(target as f32);
        }
    }

    #[func]
    fn RestartSong(&mut self) {
        self.clock.seek_seconds(0.0);
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.seek(0.0);
        }
    }

    #[func]
    fn SlowDownPlayback(&mut self) {
        self.clock.set_speed((self.clock.speed * 0.98).max(0.5));
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.set_pitch_scale(self.clock.speed as f32);
        }
    }

    #[func]
    fn SpeedUpPlayback(&mut self) {
        self.clock.set_speed((self.clock.speed / 0.98).min(1.7));
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.set_pitch_scale(self.clock.speed as f32);
        }
    }

    #[func]
    fn ResetSongSpeed(&mut self) {
        self.clock.set_speed(1.0);
        if self.has_audio_stream {
            let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
            player.set_pitch_scale(1.0);
        }
    }

    #[func]
    fn PickA(&mut self) {
        self.loop_a = Some(self.clock.song_time_seconds());
    }

    #[func]
    fn PickB(&mut self) {
        self.loop_b = Some(self.clock.song_time_seconds());
    }

    #[func]
    fn ClearLoopTimes(&mut self) {
        self.loop_a = None;
        self.loop_b = None;
    }

    #[func]
    fn MoveSongPosition(&mut self) {
        let text = self
            .base()
            .get_node_as::<LineEdit>("GridContainer/PositionSetLineEdit")
            .get_text()
            .to_string();
        if let Some(pos) = parse_song_position(&text) {
            self.clock.seek_seconds(pos.max(0.0));
            if self.has_audio_stream {
                let mut player = self.base().get_node_as::<AudioStreamPlayer>("AudioStreamPlayer");
                player.seek(pos.max(0.0) as f32);
            }
        }
    }

    #[func]
    fn GetSongPosition(&self) -> f64 {
        self.clock.song_time_seconds()
    }
}

fn calc_track_bottom_world() -> f32 {
    -0.5
}

fn calc_string_distance() -> f32 {
    1.0
}

fn calc_note_height_y(string_num: i32) -> f32 {
    (5 - string_num.clamp(0, 5)) as f32 * calc_string_distance()
}

fn calc_fret_pos_z(fret: i32) -> f32 {
    fret as f32 * 1.5
}

fn calc_in_fret_pos_z(fret: i32) -> f32 {
    calc_fret_pos_z(fret - 1) + (calc_fret_pos_z(fret) - calc_fret_pos_z(fret - 1)) / 2.0
}

fn calc_fret_width_z(fret: i32, width: i32) -> f32 {
    calc_fret_pos_z(fret + width - 1) - calc_fret_pos_z(fret - 1)
}

fn calc_middle_window_z(fret_start: i32, window_length: i32) -> f32 {
    calc_fret_pos_z(fret_start - 1) + calc_fret_width_z(fret_start, window_length) / 2.0
}

fn string_color(string_num: i32) -> Color {
    match string_num {
        0 => Color::from_rgb(1.0, 0.0, 0.0),
        1 => Color::from_rgb(1.0, 1.0, 0.0),
        2 => Color::from_rgb(0.0, 0.0, 1.0),
        3 => Color::from_rgb(1.0, 0.65, 0.0),
        4 => Color::from_rgb(0.0, 1.0, 0.0),
        _ => Color::from_rgb(0.5, 0.0, 0.5),
    }
}

fn mesh_box(color: Color, pos: Vector3, scale: Vector3) -> Gd<Node3D> {
    let mut mat = StandardMaterial3D::new_gd();
    mat.set_albedo(color);
    let mut mesh = BoxMesh::new_gd();
    mesh.set_size(scale);
    let material: Gd<Material> = mat.upcast();
    mesh.set_material(&material);

    let mut node = MeshInstance3D::new_alloc();
    let mesh_up: Gd<Mesh> = mesh.upcast();
    node.set_mesh(&mesh_up);
    node.set_position(pos);
    node.upcast()
}

fn mesh_box_translucent(color: Color, pos: Vector3, scale: Vector3) -> Gd<Node3D> {
    let mut mat = StandardMaterial3D::new_gd();
    mat.set_transparency(Transparency::ALPHA);
    mat.set_albedo(color);
    let mut mesh = BoxMesh::new_gd();
    mesh.set_size(scale);
    let material: Gd<Material> = mat.upcast();
    mesh.set_material(&material);

    let mut node = MeshInstance3D::new_alloc();
    let mesh_up: Gd<Mesh> = mesh.upcast();
    node.set_mesh(&mesh_up);
    node.set_position(pos);
    node.upcast()
}

fn box_line(color: Color, start: Vector3, end: Vector3) -> Gd<Node3D> {
    let length = (end - start).length();
    let center = start.lerp(end, 0.5);
    let dx = (end.x - start.x).abs();
    let dy = (end.y - start.y).abs();
    let dz = (end.z - start.z).abs();
    mesh_box(
        color,
        center,
        Vector3::new(
            if dx > 0.001 { length } else { 0.1 },
            if dy > 0.001 { dy.max(0.16) } else { 0.16 },
            if dz > 0.001 { dz.max(0.16) } else { 0.16 },
        ),
    )
}

fn box_line_translucent(color: Color, start: Vector3, end: Vector3) -> Gd<Node3D> {
    let length = (end - start).length();
    let center = start.lerp(end, 0.5);
    let dx = (end.x - start.x).abs();
    let dy = (end.y - start.y).abs();
    let dz = (end.z - start.z).abs();
    mesh_box_translucent(
        color,
        center,
        Vector3::new(
            if dx > 0.001 { length } else { 0.1 },
            if dy > 0.001 { dy.max(0.16) } else { 0.16 },
            if dz > 0.001 { dz.max(0.16) } else { 0.16 },
        ),
    )
}

fn basic_note(
    note: &tabplayer_parser::models::SingleNote,
    config: &tabplayer_parser::models::InstrumentConfig,
    time: f32,
    fret_window_start: i32,
    fret_window_length: i32,
) -> Gd<Node3D> {
    let color = if note.string_num == 255 {
        Color::from_rgb(1.0, 0.41, 0.71)
    } else {
        string_color(note.string_num)
    };

    if note.fret_num == 0 {
        let line_start_z = calc_fret_pos_z(fret_window_start - 1);
        let across = calc_fret_width_z(fret_window_start, fret_window_length);
        let start = Vector3::new(
            time * config.note_speed as f32,
            calc_note_height_y(note.string_num),
            line_start_z,
        );
        return box_line(color, start, start + Vector3::new(0.0, 0.0, across));
    }

    mesh_box(
        color,
        Vector3::new(
            time * config.note_speed as f32,
            calc_note_height_y(note.string_num),
            calc_in_fret_pos_z(note.fret_num),
        ),
        Vector3::new(1.25, 1.25, 1.25),
    )
}

fn make_note_geometry(
    note: &tabplayer_parser::models::SingleNote,
    config: &tabplayer_parser::models::InstrumentConfig,
    note_block: &tabplayer_parser::models::NoteBlock,
) -> Vec<Gd<Node3D>> {
    let mut out = Vec::new();

    let color = if note.string_num == 255 {
        Color::from_rgb(1.0, 0.41, 0.71)
    } else {
        string_color(note.string_num)
    };

    if !note.note_type.contains(&tabplayer_parser::models::NoteType::CHILD) {
        if note.fret_num == 0 {
            let line_start_z = calc_fret_pos_z(note_block.fret_window_start - 1);
            let across = calc_fret_width_z(note_block.fret_window_start, note_block.fret_window_length);
            let start = Vector3::new(
                note_block.time as f32 * config.note_speed as f32,
                calc_note_height_y(note.string_num),
                line_start_z,
            );
            out.push(box_line(color, start, start + Vector3::new(0.0, 0.0, across)));
        } else {
            out.push(mesh_box(
                color,
                Vector3::new(
                    note_block.time as f32 * config.note_speed as f32,
                    calc_note_height_y(note.string_num),
                    calc_in_fret_pos_z(note.fret_num),
                ),
                Vector3::new(1.25, 1.25, 1.25),
            ));
        }
    }

    if note.note_type.contains(&tabplayer_parser::models::NoteType::SUSTAIN) && note.length > 0.0 {
        let start = Vector3::new(
            note_block.time as f32 * config.note_speed as f32,
            calc_note_height_y(note.string_num),
            if note.fret_num == 0 {
                calc_middle_window_z(note_block.fret_window_start, note_block.fret_window_length)
            } else {
                calc_in_fret_pos_z(note.fret_num)
            },
        );
        let mut end = start + Vector3::new(config.note_speed as f32 * note.length as f32, 0.0, 0.0);
        if let Some(slide) = &note.slide {
            end = Vector3::new(
                end.x,
                end.y,
                calc_in_fret_pos_z(slide.to_fret),
            );
        }
        out.push(box_line(color, start, end));
    }

    let symbols = note_symbols(note);
    if !symbols.is_empty() {
        let mut text_pos = Vector3::new(
            note_block.time as f32 * config.note_speed as f32,
            calc_note_height_y(note.string_num),
            if note.fret_num == 0 {
                calc_middle_window_z(note_block.fret_window_start, note_block.fret_window_length)
            } else {
                calc_in_fret_pos_z(note.fret_num)
            },
        );
        text_pos.x -= 0.6;
        out.push(text_vertical(&symbols, text_pos));
    }

    out
}

fn note_symbols(note: &tabplayer_parser::models::SingleNote) -> String {
    let mut out = String::new();
    let t = &note.note_type;

    if t.contains(&tabplayer_parser::models::NoteType::HAMMERON) {
        out.push('h');
    }
    if t.contains(&tabplayer_parser::models::NoteType::PULLOFF) {
        out.push('p');
    }
    if t.contains(&tabplayer_parser::models::NoteType::BEND) {
        let step = note.bends.iter().map(|x| x.step as i32).max().unwrap_or(0);
        out.push('b');
        out.push_str(&step.to_string());
    }
    if t.contains(&tabplayer_parser::models::NoteType::LEFTHAND) {
        out.push('L');
    }
    if t.contains(&tabplayer_parser::models::NoteType::MUTE)
        || t.contains(&tabplayer_parser::models::NoteType::PALMMUTE)
    {
        out.push('x');
    }
    if t.contains(&tabplayer_parser::models::NoteType::TAP) {
        out.push('T');
    }
    if t.contains(&tabplayer_parser::models::NoteType::HARMONIC) {
        out.push('H');
    }
    if t.contains(&tabplayer_parser::models::NoteType::PINCHHARMONIC) {
        out.push('o');
    }
    if t.contains(&tabplayer_parser::models::NoteType::FRETHANDMUTE) {
        out.push('.');
    }

    out
}

fn text_vertical(text: &str, pos: Vector3) -> Gd<Node3D> {
    let mut label = Label3D::new_alloc();
    label.set_text(text);
    label.set_font_size(200);
    label.set_position(pos);
    let _ = label.call("rotate_y", &[std::f32::consts::FRAC_PI_2.to_variant()]);
    label.upcast()
}

fn is_same_chord_as(a: &tabplayer_parser::models::NoteBlock, b: &tabplayer_parser::models::NoteBlock) -> bool {
    if a.notes.len() != b.notes.len() {
        return false;
    }
    if a.chord_flags != b.chord_flags {
        return false;
    }

    for (na, nb) in a.notes.iter().zip(b.notes.iter()) {
        if na.fret_num != nb.fret_num || na.string_num != nb.string_num {
            return false;
        }
        if !na.bends.is_empty() || na.slide.is_some() || !nb.bends.is_empty() || nb.slide.is_some() {
            return false;
        }

        let mut ta = na
            .note_type
            .iter()
            .copied()
            .filter(|x| {
                !matches!(
                    x,
                    tabplayer_parser::models::NoteType::UNDEFINED
                        | tabplayer_parser::models::NoteType::MISSING
                        | tabplayer_parser::models::NoteType::CHORD
                        | tabplayer_parser::models::NoteType::OPEN
                        | tabplayer_parser::models::NoteType::IGNORE
                        | tabplayer_parser::models::NoteType::HIGHDENSITY
                        | tabplayer_parser::models::NoteType::SINGLE
                        | tabplayer_parser::models::NoteType::CHORDNOTES
                        | tabplayer_parser::models::NoteType::DOUBLESTOP
                        | tabplayer_parser::models::NoteType::MISSING2
                        | tabplayer_parser::models::NoteType::STRUM
                        | tabplayer_parser::models::NoteType::ACCENT
                )
            })
            .collect::<Vec<_>>();
        let mut tb = nb
            .note_type
            .iter()
            .copied()
            .filter(|x| {
                !matches!(
                    x,
                    tabplayer_parser::models::NoteType::UNDEFINED
                        | tabplayer_parser::models::NoteType::MISSING
                        | tabplayer_parser::models::NoteType::CHORD
                        | tabplayer_parser::models::NoteType::OPEN
                        | tabplayer_parser::models::NoteType::IGNORE
                        | tabplayer_parser::models::NoteType::HIGHDENSITY
                        | tabplayer_parser::models::NoteType::SINGLE
                        | tabplayer_parser::models::NoteType::CHORDNOTES
                        | tabplayer_parser::models::NoteType::DOUBLESTOP
                        | tabplayer_parser::models::NoteType::MISSING2
                        | tabplayer_parser::models::NoteType::STRUM
                        | tabplayer_parser::models::NoteType::ACCENT
                )
            })
            .collect::<Vec<_>>();
        ta.sort_by_key(|x| *x as i32);
        tb.sort_by_key(|x| *x as i32);
        if ta != tb {
            return false;
        }
    }

    true
}

fn generate_note_block_frets(instrument: &tabplayer_parser::models::SongInstrument) -> Vec<Gd<Node3D>> {
    let mut out = Vec::new();
    let mut start_of_section = -10.0_f64;
    let mut cur_start = -1;
    let mut cur_length = -1;

    for note in &instrument.notes {
        if cur_start != note.fret_window_start || cur_length != note.fret_window_length {
            if cur_start > 0 {
                while start_of_section < note.time {
                    let length = (note.time - start_of_section).min(10.0);
                    out.push(create_window_piece(
                        cur_start,
                        cur_length,
                        (start_of_section + length) as f32,
                        start_of_section as f32,
                        instrument,
                    ));
                    start_of_section += length;
                }
            }
            cur_start = note.fret_window_start;
            cur_length = note.fret_window_length;
            start_of_section = note.time;
        }
    }

    if cur_start > 0 {
        if let Some(last) = instrument.notes.last() {
            while start_of_section < last.time + 1.0 {
                let length = ((last.time + 1.0) - start_of_section).min(10.0);
                out.push(create_window_piece(
                    cur_start,
                    cur_length,
                    (start_of_section + length) as f32,
                    start_of_section as f32,
                    instrument,
                ));
                start_of_section += length;
            }
        }
    }

    out
}

fn create_window_piece(
    fret: i32,
    length: i32,
    end_time: f32,
    start_time: f32,
    instrument: &tabplayer_parser::models::SongInstrument,
) -> Gd<Node3D> {
    let across = calc_fret_width_z(fret, length);
    let pos = Vector3::new(
        ((end_time + start_time) / 2.0) * instrument.config.note_speed as f32 - 0.5,
        calc_track_bottom_world() - 0.01,
        calc_fret_pos_z(fret - 1) + across / 2.0,
    );

    let mut mat = StandardMaterial3D::new_gd();
    mat.set_transparency(Transparency::ALPHA);
    mat.set_albedo(Color::from_rgba(0.28, 0.24, 0.55, 0.35));
    let mut plane = PlaneMesh::new_gd();
    plane.set_size(Vector2::new(
        instrument.config.note_speed as f32 * (end_time - start_time),
        across,
    ));
    let material: Gd<Material> = mat.upcast();
    plane.set_material(&material);
    let mut node = MeshInstance3D::new_alloc();
    let plane_mesh: Gd<Mesh> = plane.upcast();
    node.set_mesh(&plane_mesh);
    node.set_position(pos);
    node.upcast()
}

fn to_min_sec_msec(value: f64, frac: bool) -> String {
    let min = (value / 60.0).floor() as i64;
    let sec = (value % 60.0).floor() as i64;
    if frac {
        let ms = ((value.fract().abs()) * 1000.0).floor() as i64;
        format!("{min}m {sec:02}s {ms:03}ms")
    } else {
        format!("{min}m {sec:02}s")
    }
}

fn parse_song_position(text: &str) -> Option<f64> {
    if let Ok(v) = text.trim().parse::<f64>() {
        return Some(v);
    }

    let cleaned = text
        .replace("ms", " ")
        .replace('m', " ")
        .replace('s', " ")
        .replace(':', " ");
    let nums = cleaned
        .split_whitespace()
        .filter_map(|x| x.parse::<f64>().ok())
        .collect::<Vec<_>>();

    match nums.as_slice() {
        [m, s, ms] => Some(m * 60.0 + s + ms / 1000.0),
        [m, s] => Some(m * 60.0 + s),
        [s] => Some(*s),
        _ => None,
    }
}
