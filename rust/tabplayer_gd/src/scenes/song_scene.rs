use super::shared::*;
use super::{calculate_song_position, GuitarChart, NoteMiniGraph, SongChart};

#[derive(GodotClass)]
#[class(base=Node)]
pub struct SongScene {
    #[base]
    base: Base<Node>,
    state: Option<SongState>,
    audio_stream: Option<Gd<AudioStreamWav>>,
    player: Option<Gd<AudioStreamPlayer>>,
    cached_song_position: Cell<Option<f64>>,
    note_graph_scene: Option<Gd<NoteMiniGraph>>,
    song_chart_scene: Option<Gd<SongChart>>,
    a_position: f64,
    b_position: f64,
}

#[godot_api]
impl INode for SongScene {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            state: None,
            audio_stream: None,
            player: None,
            cached_song_position: Cell::new(None),
            note_graph_scene: None,
            song_chart_scene: None,
            a_position: 0.0,
            b_position: 0.0,
        }
    }

    fn ready(&mut self) {
        if let Some(state) = &self.state {
            self.set_ui_labels(state);
            if let Some(mut player) = self.base_mut().try_get_node_as::<AudioStreamPlayer>("AudioStreamPlayer") {
                if let Some(stream) = &self.audio_stream {
                    player.set_stream(Some(&stream.clone().upcast::<AudioStream>()));
                }
                player.play();
                self.player = Some(player.clone());
            }

            let player = self.player.clone();
            let mut guitar_chart = Gd::<GuitarChart>::from_init_fn(|mut chart| {
                chart.state = Some(state.clone());
                chart.audio_player = player.clone();
            });
            self.base_mut().add_child(Some(&guitar_chart.clone().upcast::<Node>()));

            self.load_instrument_from_state();
        }
    }

    fn process(&mut self, delta: f64) {
        self.cached_song_position.set(None);
        let song_position = self.get_song_position();
        if let Some(mut label) = self.base_mut().try_get_node_as::<Label>("GridContainer/ABLabelStart") {
            let text = if self.a_position == 0.0 {
                String::new()
            } else {
                to_min_sec(self.a_position, true)
            };
            label.set_text(text.as_str());
        }
        if let Some(mut label) = self.base_mut().try_get_node_as::<Label>("GridContainer/ABLabelEnd") {
            let text = if self.b_position == 0.0 {
                String::new()
            } else {
                to_min_sec(self.b_position, true)
            };
            label.set_text(text.as_str());
        }
        if self.a_position != 0.0 && self.b_position != 0.0 {
            if self.a_position < song_position && delta + song_position > self.b_position {
                if let Some(player) = self.player.as_mut() {
                    player.set_stream_paused(false);
                    player.seek(self.a_position as f32);
                }
            }
        }
        if let Some(player) = &self.player {
            if !player.is_playing() {
                return;
            }
        }
        if let Some(state) = &self.state {
            if let Some(next_note) = self.next_note_block(state, song_position) {
                if let Some(mut label) = self.base_mut().try_get_node_as::<Label>("GridContainer/SkipToNextLabel2") {
                    label.set_text(&format!("at {}", to_min_sec(next_note.time as f64, false)));
                }
            }
            if let Some(mut details) = self.base_mut().try_get_node_as::<Label>("DetailsVBoxContainer/SongDetailsLabel") {
                let instrument = state.instrument();
                let note_text = if let Some(next_note) = self.next_note_block(state, song_position) {
                    format!(
                        "Next Note: {} in {:.1}",
                        to_min_sec(next_note.time as f64, false),
                        next_note.time as f64 - song_position
                    )
                } else {
                    "No note".to_string()
                };
                details.set_text(&format!(
                    "---------\nTuning: {}\nNotes: {}\nChords: {}\nFirst note @ {}\nLast note @ {}\n---------\n{note_text}\n",
                    Instrument::calc_tuning_name(instrument.config.tuning, instrument.config.capo_fret),
                    instrument.single_note_count(),
                    instrument.chord_count(),
                    to_min_sec(instrument.notes.first().map(|n| n.time as f64).unwrap_or(0.0), false),
                    to_min_sec(instrument.notes.last().map(|n| n.time as f64).unwrap_or(0.0), false),
                ));
            }
            if let Some(mut label) = self.base_mut().try_get_node_as::<Label>("RunningDetailsLabel") {
                label.set_text(&format!(
                    "{}fps | {:03.1}ms\n{}",
                    Engine::get_frames_per_second(),
                    delta * 1000.0,
                    to_min_sec(song_position, true)
                ));
            }
            if let Some(mut lyrics_label) = self.base_mut().try_get_node_as::<RichTextLabel>("HBoxContainer/LyricsLabel") {
                self.update_lyrics(state, song_position, &mut lyrics_label);
            }
            if let Some(mut pos_line) = self.base_mut().try_get_node_as::<LineEdit>("GridContainer/PositionSetLineEdit") {
                pos_line.set_text(&to_min_sec(song_position, true));
            }
            if let Some(mut speed_label) = self.base_mut().try_get_node_as::<Label>("GridContainer/SongSpeedLabel") {
                if let Some(player) = &self.player {
                    speed_label.set_text(&format!("{:.1}%", player.get_pitch_scale() * 100.0));
                }
            }
        }
    }
}

#[godot_api]
impl SongScene {
    #[signal]
    fn closed();

    pub fn init(&mut self, state: SongState) {
        let mut stream = AudioStreamWav::new_gd();
        stream.set_format(audio_stream_wav::Format::FORMAT_16_BITS);
        stream.set_mix_rate(state.audio_sample_rate);
        stream.set_stereo(state.audio_channels >= 2);
        stream.set_data(PackedByteArray::from(state.audio.clone()));
        self.audio_stream = Some(stream);
        self.state = Some(state);
    }

    #[func]
    #[allow(non_snake_case)]
    fn InstrumentChanged(&mut self, id: i64) {
        if let Some(state) = &mut self.state {
            let instrument = state
                .song_info
                .instruments
                .get(id as usize)
                .map(|inst| inst.name.clone());
            if let Some(name) = instrument {
                state.instrument_name = name.clone();
                if let Some(mut list) = self.base_mut().try_get_node_as::<MenuButton>("DetailsVBoxContainer/VBoxContainer/InstrumentMenuButton") {
                    list.set_text(name.as_str());
                }
                self.load_instrument_from_state();
            }
        }
    }

    fn load_instrument_from_state(&mut self) {
        if let Some(mut note_graph) = self.note_graph_scene.take() {
            self.base_mut().remove_child(Some(&note_graph.clone().upcast::<Node>()));
        }
        if let Some(state) = &self.state {
            let player = self.player.clone();
            let mut note_graph = Gd::<NoteMiniGraph>::from_init_fn(|mut graph| {
                graph.song_state = Some(state.clone());
                graph.audio_player = player.clone();
            });
            self.base_mut().add_child(Some(&note_graph.clone().upcast::<Node>()));
            self.note_graph_scene = Some(note_graph);

            if let Some(mut song_chart) = self.song_chart_scene.take() {
                self.base_mut().remove_child(Some(&song_chart.clone().upcast::<Node>()));
            }
            let mut song_chart = Gd::<SongChart>::from_init_fn(|mut chart| {
                chart.instrument = Some(state.instrument().clone());
            });
            self.base_mut().add_child(Some(&song_chart.clone().upcast::<Node>()));
            self.song_chart_scene = Some(song_chart);
        }
    }

    fn set_ui_labels(&mut self, state: &SongState) {
        if let Some(mut info_label) = self.base_mut().try_get_node_as::<Label>("DetailsVBoxContainer/SongInfoLabel") {
            info_label.set_text(&format!(
                "{} ({:?})\n{}\n{}",
                state.song_info.metadata.name,
                state.song_info.metadata.year,
                state.song_info.metadata.artist,
                state.song_info.metadata.album
            ));
        }
        if let Some(mut instrument_list) = self.base_mut().try_get_node_as::<MenuButton>("DetailsVBoxContainer/VBoxContainer/InstrumentMenuButton") {
            instrument_list.set_text(state.instrument().name.as_str());
            if let Some(mut popup) = instrument_list.get_popup() {
                popup.clear();
                for instrument in &state.song_info.instruments {
                    popup.add_item(instrument.name.clone());
                }
                popup.set_item_checked(state.song_info.main_instrument_index as i32, true);
                popup.connect("id_pressed", &self.base_mut().callable("InstrumentChanged"));
            }
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn PauseButton_Pressed(&mut self) {
        if let Some(player) = self.player.as_ref() {
            if player.is_stream_paused() {
                self.resume();
            } else {
                self.pause();
            }
        }
    }

    fn pause(&mut self) {
        if let Some(player) = self.player.as_mut() {
            player.set_stream_paused(true);
        }
    }

    fn resume(&mut self) {
        if let Some(player) = self.player.as_mut() {
            player.set_stream_paused(false);
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn Quit(&mut self) {
        self.pause();
        if let Some(player) = self.player.as_mut() {
            player.stop();
        }
        self.base_mut().emit_signal("closed", &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn Skip10Sec(&mut self) {
        self.skip_10_sec();
    }

    #[func]
    #[allow(non_snake_case)]
    fn Back10Sec(&mut self) {
        self.back_10_sec();
    }

    #[func]
    #[allow(non_snake_case)]
    fn SkipToNext(&mut self) {
        self.skip_to_next();
    }

    #[func]
    #[allow(non_snake_case)]
    fn RestartSong(&mut self) {
        self.restart_song();
    }

    #[func]
    #[allow(non_snake_case)]
    fn SlowDownPlayback(&mut self) {
        self.slow_down();
    }

    #[func]
    #[allow(non_snake_case)]
    fn SpeedUpPlayback(&mut self) {
        self.speed_up();
    }

    #[func]
    #[allow(non_snake_case)]
    fn ResetSongSpeed(&mut self) {
        self.reset_song_speed();
    }

    #[func]
    #[allow(non_snake_case)]
    fn PickA(&mut self) {
        self.pick_a();
    }

    #[func]
    #[allow(non_snake_case)]
    fn PickB(&mut self) {
        self.pick_b();
    }

    #[func]
    #[allow(non_snake_case)]
    fn ClearLoopTimes(&mut self) {
        self.clear_loop_times();
    }

    #[func]
    #[allow(non_snake_case)]
    fn SongFinished(&mut self) {
        if let Some(player) = self.player.as_mut() {
            player.play();
            player.seek(0.0);
            player.set_stream_paused(true);
            self.pause();
        }
    }

    #[func]
    fn _input(&mut self, event: Gd<InputEvent>) {
        if event.is_action_pressed("ui_cancel") {
            self.Quit();
        } else if event.is_action_pressed("song_pause") {
            self.PauseButton_Pressed();
        } else if event.is_action_pressed("song_skip_forward_10") {
            self.skip_10_sec();
        } else if event.is_action_pressed("song_skip_backward_10") {
            self.back_10_sec();
        } else if event.is_action_pressed("song_skip_to_next") {
            self.skip_to_next();
        } else if event.is_action_pressed("song_restart") {
            self.restart_song();
        } else if event.is_action_pressed("song_speed_down") {
            self.slow_down();
        } else if event.is_action_pressed("song_speed_up") {
            self.speed_up();
        } else if event.is_action_pressed("song_set_loop_start") {
            self.pick_a();
        } else if event.is_action_pressed("song_set_loop_end") {
            self.pick_b();
        } else if event.is_action_pressed("song_reset_loop") {
            self.clear_loop_times();
        } else if event.is_action_pressed("song_reset_speed") {
            self.reset_song_speed();
        }
    }

    fn skip_10_sec(&mut self) {
        let pos = self.get_song_position() + 10.0;
        if let Some(player) = self.player.as_mut() {
            player.seek(pos as f32);
        }
    }

    fn back_10_sec(&mut self) {
        let pos = self.get_song_position() - 10.0;
        if let Some(player) = self.player.as_mut() {
            player.seek(pos as f32);
        }
    }

    fn skip_to_next(&mut self) {
        if let Some(state) = &self.state {
            if let Some(next_note) = self.next_note_block(state, self.get_song_position()) {
                if let Some(player) = self.player.as_mut() {
                    player.seek(next_note.time - 1.5);
                }
            }
        }
    }

    fn restart_song(&mut self) {
        if let Some(player) = self.player.as_mut() {
            player.seek(0.0);
        }
    }

    fn slow_down(&mut self) {
        self.adjust_pitch(0.98);
    }

    fn speed_up(&mut self) {
        self.adjust_pitch(1.0 / 0.98);
    }

    fn adjust_pitch(&mut self, amount: f32) {
        if let Some(player) = self.player.as_ref() {
            if player.get_pitch_scale() < 0.5 && amount < 1.0 {
                return;
            }
            if player.get_pitch_scale() > 1.7 && amount > 1.0 {
                return;
            }
            self.set_song_speed(player.get_pitch_scale() * amount);
        }
    }

    fn reset_song_speed(&mut self) {
        self.set_song_speed(1.0);
    }

    fn set_song_speed(&mut self, fraction: f32) {
        if let Some(player) = self.player.as_mut() {
            player.set_pitch_scale(fraction);
            if fraction == 1.0 {
                player.set_bus("Master");
            } else {
                player.set_bus("SongPlayback");
            }
            let bus_id = AudioServer::singleton().get_bus_index("SongPlayback");
            if let Some(mut effect) = AudioServer::singleton()
                .get_bus_effect(bus_id, 0)
                .and_then(|e| e.try_cast::<AudioEffectPitchShift>())
            {
                effect.set_pitch_scale(1.0 / player.get_pitch_scale());
            }
        }
    }

    fn pick_a(&mut self) {
        self.a_position = self.get_song_position();
    }

    fn pick_b(&mut self) {
        self.b_position = self.get_song_position();
    }

    fn clear_loop_times(&mut self) {
        self.a_position = 0.0;
        self.b_position = 0.0;
    }

    #[func]
    #[allow(non_snake_case)]
    fn MoveSongPosition(&mut self) {
        if let Some(mut line_edit) = self.base_mut().try_get_node_as::<LineEdit>("GridContainer/PositionSetLineEdit") {
            let text = line_edit.get_text().to_string();
            let mut pos = text.parse::<f32>().unwrap_or(0.0);
            if pos == 0.0 {
                if let Some(parts) = parse_min_sec(&text) {
                    pos = parts;
                }
            }
            if pos <= 0.0 {
                return;
            }
            if let Some(player) = self.player.as_mut() {
                if pos > player.get_stream().map(|s| s.get_length()).unwrap_or(0.0) {
                    return;
                }
                player.set_stream_paused(false);
                player.seek(pos);
                self.pause();
            }
        }
    }

    pub fn get_song_position(&self) -> f64 {
        if let Some(cached) = self.cached_song_position.get() {
            return cached;
        }
        if let Some(player) = &self.player {
            let mut time = calculate_song_position(player);
            time -= SettingsService::settings().audio_position_offset_ms / 1000.0;
            self.cached_song_position.set(Some(time));
            return time;
        }
        0.0
    }

    fn next_note_block<'a>(&self, state: &'a SongState, song_pos: f64) -> Option<&'a NoteBlock> {
        state
            .instrument()
            .notes
            .iter()
            .find(|n| n.time as f64 > song_pos)
    }

    fn update_lyrics(&self, state: &SongState, song_pos: f64, label: &mut Gd<RichTextLabel>) {
        label.clear();
        label.push_font_size(40);
        let lines = current_lines(state, song_pos);
        if lines.is_empty() {
            label.set_text("");
            return;
        }
        if let Some(line) = lines.get(0) {
            let (part_a, part_b) = line.get_parts(song_pos);
            label.push_color(Color::from_rgb(1.0, 0.0, 0.0));
            label.add_text(part_a.into());
            label.push_color(Color::from_rgb(1.0, 1.0, 0.0));
            label.add_text(part_b.into());
        }
        if let Some(line) = lines.get(1) {
            label.add_text(format!("\n{}", line.text()));
        }
    }
}
fn parse_min_sec(text: &str) -> Option<f32> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let minutes = parts.get(0)?.trim_end_matches('m').parse::<f32>().ok()?;
    let seconds = parts.get(1)?.trim_end_matches('s').parse::<f32>().ok()?;
    let millis = parts.get(2)?.trim_end_matches("ms").parse::<f32>().ok()?;
    Some(minutes * 60.0 + seconds + millis / 1000.0)
}
fn current_lines(state: &SongState, song_pos: f64) -> Vec<LyricLine> {
    if state.song_info.lyrics.lines.is_empty() {
        return Vec::new();
    }
    if state.song_info.lyrics.lines.first().map(|line| line.start_time as f64 > song_pos).unwrap_or(false) {
        return state.song_info.lyrics.lines.iter().take(2).cloned().collect();
    }
    for line in &state.song_info.lyrics.lines {
        if line.end_time as f64 >= song_pos {
            return state.song_info.lyrics.lines.iter().skip_while(|x| x.start_time != line.start_time).take(2).cloned().collect();
        }
    }
    Vec::new()
}
