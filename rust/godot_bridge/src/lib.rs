#![allow(non_snake_case)]

mod common;
mod convert_menu;
mod guitar_chart;
mod info_page;
mod main_scene;
mod note_bucket_graph;
mod note_mini_graph;
mod settings_page;
mod song_chart;
mod song_display;
mod song_list;
mod song_pick;
mod song_scene;
mod start_menu;
mod tabplayer_backend;

use godot::prelude::*;

struct TabPlayerExtension;

#[gdextension]
unsafe impl ExtensionLibrary for TabPlayerExtension {}
