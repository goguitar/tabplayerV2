use crate::common::*;

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
struct SongList {
    #[base]
    _base: Base<VBoxContainer>,
}

#[godot_api]
impl IVBoxContainer for SongList {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { _base: base }
    }
}

#[godot_api]
impl SongList {
    #[func]
    fn SelectRandom(&mut self) {}

    #[func]
    fn UpdateFilter(&mut self, _filter: GString) {}

    #[func]
    fn TuningSelected(&mut self, _index: i64) {}

    #[func]
    fn ShowCapo_Pressed(&mut self) {}
}
