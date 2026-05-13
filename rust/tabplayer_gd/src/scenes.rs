mod shared;
mod main_scene;
mod start_menu;
mod convert_menu;
mod info_page;
mod settings_page;
mod song_pick;
mod song_display;
mod song_list;
mod song_scene;
mod guitar_chart;
mod note_mini_graph;
mod note_bucket_graph;
mod song_chart;

pub use main_scene::MainScene;
pub use start_menu::StartMenu;
pub use convert_menu::ConvertMenu;
pub use info_page::InfoPage;
pub use settings_page::SettingsPage;
pub use song_pick::SongPick;
pub use song_display::SongDisplay;
pub use song_list::SongList;
pub use song_scene::SongScene;
pub use guitar_chart::GuitarChart;
pub use note_mini_graph::NoteMiniGraph;
pub use note_bucket_graph::NoteBucketGraph;
pub use song_chart::SongChart;

use godot::classes::{AudioServer, AudioStreamPlayer, PackedScene};
use godot::prelude::*;

pub(crate) fn load_scene<T: GodotClass + Inherits<Node>>(path: &str) -> Gd<T> {
    let packed = load::<PackedScene>(path);
    packed.instantiate_as::<T>()
}

pub(crate) fn calculate_song_position(player: &Gd<AudioStreamPlayer>) -> f64 {
    let mut time =
        player.get_playback_position() as f64 + AudioServer::singleton().get_time_since_last_mix() as f64;
    time -= AudioServer::singleton().get_output_latency() as f64;
    time
}
