use godot::prelude::*;

mod models;
mod scenes;
mod services;
mod song_repository;
mod vgmstream;

struct TabPlayerExtension;

#[gdextension]
unsafe impl ExtensionLibrary for TabPlayerExtension {}
