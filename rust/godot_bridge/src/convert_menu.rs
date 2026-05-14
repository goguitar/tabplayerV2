use crate::common::*;

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
struct ConvertMenu {
    #[base]
    base: Base<VBoxContainer>,
}

#[godot_api]
impl IVBoxContainer for ConvertMenu {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { base }
    }
}

#[godot_api]
impl ConvertMenu {
    #[func]
    fn ChoseButton_Pressed(&mut self) {
        let mut dialog = self.base().get_node_as::<FileDialog>("FileDialog");
        dialog.popup_centered();
    }

    #[func]
    fn FromDownloadsButton_Pressed(&mut self) {
        self.rescan_dlc(PathBuf::from("/home/csantz/Music/DLC"));
    }

    #[func]
    fn Dir_Selected(&mut self, dir: GString) {
        self.rescan_dlc(PathBuf::from(dir.to_string()));
    }

    #[func]
    fn File_Selected(&mut self, path: GString) {
        let path = PathBuf::from(path.to_string());
        let dir = if path.is_dir() {
            path
        } else {
            path.parent().map(|x| x.to_path_buf()).unwrap_or(path)
        };
        self.rescan_dlc(dir);
    }

    #[func]
    fn Files_Selected(&mut self, paths: PackedStringArray) {
        if let Some(first) = paths.as_slice().first() {
            self.File_Selected(first.clone());
        }
    }

    #[func]
    fn BackButton_Pressed(&mut self) {
        let mut tree = self.base().get_tree();
        let _ = tree.change_scene_to_file("res://scenes/StartMenu.tscn");
    }

    #[func]
    fn AnimateIn(&mut self) {}

    #[func]
    fn AnimateOut(&mut self) {}

    fn rescan_dlc(&mut self, dir: PathBuf) {
        let mut info = self.base().get_node_as::<Label>("InfoLabel");
        match catalog_rescan_dlc_dir(&dir) {
            Ok(count) => {
                let msg = format!("Scanned {} songs from {}", count, dir.to_string_lossy());
                info.set_text(&msg);
            }
            Err(err) => {
                let msg = format!("Rescan failed: {err}");
                info.set_text(&msg);
            }
        }
    }
}
