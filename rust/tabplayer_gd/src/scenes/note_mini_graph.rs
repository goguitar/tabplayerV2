use super::shared::*;
use godot::classes::image;
use super::calculate_song_position;

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct NoteMiniGraph {
    #[base]
    base: Base<Node2D>,
    pub song_state: Option<SongState>,
    pub audio_player: Option<Gd<AudioStreamPlayer>>,
    note_plot_image: Option<Gd<Texture2D>>,
}
#[godot_api]
impl INode2D for NoteMiniGraph {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            song_state: None,
            audio_player: None,
            note_plot_image: None,
        }
    }

    fn ready(&mut self) {
        let song_state = self.song_state.clone();
        if let Some(state) = song_state {
            let string_colours = SettingsService::settings().string_colours;
            let window = self.base_mut().get_viewport().unwrap().get_visible_rect();
            let left_offset = (0.07 * window.size.x) as i32;
            let note_offset = ((1.0 - 0.07 * 2.0) * window.size.x) as i32;
            let mut image = match Image::create_empty(
                window.size.x as i32,
                window.size.y as i32,
                false,
                image::Format::RGBA8,
            ) {
                Some(image) => image,
                None => return,
            };
            image.fill_rect(
                Rect2i::new(Vector2i::new(left_offset, window.size.y as i32 - 30 - 25 * 3), Vector2i::new(note_offset, window.size.y as i32 - 30)),
                Color::from_rgba(1.0, 1.0, 1.0, 0.05),
            );
            for note_block in &state.instrument().notes {
                let pos_x = left_offset as f32 + note_offset as f32 * note_block.time / state.song_info.metadata.song_length;
                for note in &note_block.notes {
                    let pos_y = window.size.y as i32 - note.fret_num * 3 - 30;
                    draw_note_pixel(&mut image, string_colours[note.string_num as usize], Vector2i::new(pos_x as i32, pos_y));
                }
            }
            if let Some(texture) = ImageTexture::create_from_image(&image) {
                self.note_plot_image = Some(texture.upcast());
            }
        }
    }

    fn draw(&mut self) {
        let note_plot_image = self.note_plot_image.clone();
        if let Some(texture) = note_plot_image {
            self.base_mut().draw_texture(&texture, Vector2::ZERO);
        }
        let song_state = self.song_state.clone();
        let audio_player = self.audio_player.clone();
        if let (Some(state), Some(player)) = (song_state, audio_player) {
            let window = self.base_mut().get_viewport().unwrap().get_visible_rect();
            let left_offset = (0.07 * window.size.x) as f32;
            let note_offset = (1.0 - 0.07 * 2.0) * window.size.x;
            let pos_x = left_offset + note_offset * calculate_song_position(&player) as f32 / state.song_info.metadata.song_length;
            self.base_mut().draw_line(
                Vector2::new(pos_x, window.size.y - 30.0),
                Vector2::new(pos_x, window.size.y - 30.0 - 24.0 * 3.0),
                Color::from_rgb(1.0, 1.0, 1.0),
            );
        }
    }

    fn process(&mut self, _delta: f64) {
        if let Some(player) = &self.audio_player {
            if calculate_song_position(player) > 0.0 {
                self.base_mut().queue_redraw();
            }
        }
    }
}

impl NoteMiniGraph {
    pub fn from_state(
        base: Base<Node2D>,
        song_state: SongState,
        audio_player: Option<Gd<AudioStreamPlayer>>,
    ) -> Self {
        Self {
            base,
            song_state: Some(song_state),
            audio_player,
            note_plot_image: None,
        }
    }
}
fn draw_note_pixel(image: &mut Gd<Image>, colour: Color, pos: Vector2i) {
    for i in -1..1 {
        for j in -1..1 {
            image.set_pixel(pos.x + i, pos.y + j, colour);
        }
    }
}
