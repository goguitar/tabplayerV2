use crate::common::*;

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
struct SongDisplay {
    #[base]
    _base: Base<VBoxContainer>,
}

#[godot_api]
impl IVBoxContainer for SongDisplay {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { _base: base }
    }
}
