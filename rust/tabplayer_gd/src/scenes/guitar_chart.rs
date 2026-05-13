use super::shared::*;
use super::calculate_song_position;

#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct GuitarChart {
    #[base]
    base: Base<Node3D>,
    pub state: Option<SongState>,
    pub audio_player: Option<Gd<AudioStreamPlayer>>,
    last_note_block_time: Option<f32>,
    last_note_block_node: Option<Gd<Node3D>>,
}

#[godot_api]
impl INode3D for GuitarChart {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            state: None,
            audio_player: None,
            last_note_block_time: None,
            last_note_block_node: None,
        }
    }

    fn ready(&mut self) {
        let mut material = StandardMaterial3D::new_gd();
        material.set_albedo(Color::from_rgb(0.82, 0.71, 0.55));
        let mut plane_mesh = PlaneMesh::new_gd();
        plane_mesh.set_size(Vector2::new(6.0, 6.0));
        plane_mesh.set_center_offset(Vector3::new(2.5, 0.0, -3.0));
        plane_mesh.set_material(Some(&material.upcast::<Material>()));
        let mut plane = MeshInstance3D::new_alloc();
        plane.set_transform(Transform3D::new(
            Basis::from_rows(Vector3::new(0.0, -1.0, 0.0), Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 1.0)),
            Vector3::ZERO,
        ));
        plane.set_mesh(Some(&plane_mesh.upcast::<Mesh>()));
        self.base_mut().add_child(Some(&plane.upcast::<Node>()));

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
        self.base_mut().add_child(Some(&camera.upcast::<Node>()));

        let mut light = DirectionalLight3D::new_alloc();
        light.set_transform(Transform3D::new(
            Basis::from_rows(
                Vector3::new(-0.177838, 0.752991, -0.633544),
                Vector3::new(-0.317607, 0.565433, 0.761191),
                Vector3::new(0.931397, 0.336587, 0.1386),
            ),
            Vector3::ZERO,
        ));
        self.base_mut().add_child(Some(&light.upcast::<Node>()));

        if let Some(state) = &self.state {
            for i in 0..6 {
                let colour = SettingsService::get_color_from_string_num(i);
                let mut string_material = StandardMaterial3D::new_gd();
                string_material.set_albedo(colour);
                let mut string_mesh = BoxMesh::new_gd();
                string_mesh.set_size(Vector3::new(0.08, 0.08, 50.0));
                string_mesh.set_material(Some(&string_material.upcast::<Material>()));
                let mut string_obj = MeshInstance3D::new_alloc();
                string_obj.set_transform(Transform3D::new(
                    Basis::IDENTITY,
                    Vector3::new(0.0, DisplayConst::calc_note_height_y(i as i32), 25.0),
                ));
                string_obj.set_mesh(Some(&string_mesh.upcast::<Mesh>()));
                self.base_mut().add_child(Some(&string_obj.upcast::<Node>()));
            }

            let mut fret_material = StandardMaterial3D::new_gd();
            fret_material.set_albedo(Color::from_rgb(0.82, 0.71, 0.55));
            let mut fret_mesh = BoxMesh::new_gd();
            fret_mesh.set_size(Vector3::new(0.03, 5.0 + DisplayConst::TRACK_BOTTOM_WORLD.abs() * 2.0, 0.03));
            fret_mesh.set_material(Some(&fret_material.upcast::<Material>()));
            for i in 0..25 {
                let mut fret_obj = MeshInstance3D::new_alloc();
                fret_obj.set_transform(Transform3D::new(
                    Basis::IDENTITY,
                    Vector3::new(0.0, 2.5, DisplayConst::calc_fret_pos_z(i)),
                ));
                fret_obj.set_mesh(Some(&fret_mesh.clone().upcast::<Mesh>()));
                self.base_mut().add_child(Some(&fret_obj.upcast::<Node>()));
            }
        }
    }

    fn process(&mut self, delta: f64) {
        let song_pos = self.audio_player.as_ref().map(|player| calculate_song_position(player)).unwrap_or(0.0);
        let state = self.state.clone();
        if let Some(state) = state {
            if let Some(note_block) = state.instrument().notes.iter().find(|b| b.time as f64 > song_pos) {
                let tree = self.base_mut().get_tree();
                if let Some(mut cam) = tree.get_root().and_then(|root| root.get_camera_3d()) {
                    let delta = delta as f32;
                    let cam_move_speed = SettingsService::settings().camera_aim_speed / 50.0;
                    let want_pos = DisplayConst::calc_middle_window_z(note_block.fret_window_start, note_block.fret_window_length);
                    let cam_pos = cam.get_position();
                    let new_z = cam_pos.z * (1.0 - delta * cam_move_speed) + want_pos * delta * cam_move_speed;
                    cam.set_position(Vector3::new(cam_pos.x, cam_pos.y, new_z));
                }
                if self.last_note_block_time != Some(note_block.time) {
                    self.last_note_block_time = Some(note_block.time);
                    if let Some(mut node) = self.last_note_block_node.take() {
                        node.queue_free();
                    }
                    let mut block_node = Node3D::new_alloc();
                    self.base_mut().add_child(Some(&block_node.clone().upcast::<Node>()));
                    for note in &note_block.notes {
                        let note_node = NoteGenerator::get_basic_note(
                            note,
                            state.instrument().config,
                            0.2 / state.instrument().config.note_speed,
                            note_block.fret_window_start,
                            note_block.fret_window_length,
                        );
                        block_node.add_child(Some(&note_node.upcast::<Node>()));
                    }
                    self.last_note_block_node = Some(block_node);
                }
            }
        }
    }
}

impl GuitarChart {
    pub fn from_state(
        base: Base<Node3D>,
        state: SongState,
        audio_player: Option<Gd<AudioStreamPlayer>>,
    ) -> Self {
        Self {
            base,
            state: Some(state),
            audio_player,
            last_note_block_time: None,
            last_note_block_node: None,
        }
    }
}
