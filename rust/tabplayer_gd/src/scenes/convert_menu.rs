use super::shared::*;

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
pub struct ConvertMenu {
    #[base]
    base: Base<VBoxContainer>,
    tween: Option<TweenHelper>,
}

#[godot_api]
impl IVBoxContainer for ConvertMenu {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { base, tween: None }
    }

    fn ready(&mut self) {
        let tween = {
            let mut base = self.base_mut();
            let initial_pos = Vector2::new(-base.get_size().x, base.get_position().y);
            let tween = TweenHelper::new(
                base.get_tree(),
                base.clone().upcast(),
                "position",
                initial_pos.to_variant(),
                base.get_position().to_variant(),
            );
            base.set_position(initial_pos);
            tween
        };
        self.tween = Some(tween);
    }
}

#[godot_api]
impl ConvertMenu {
    #[signal]
    fn closed();

    #[func]
    #[allow(non_snake_case)]
    fn ChoseButton_Pressed(&mut self) {
        if let Some(mut info_label) = self.base_mut().try_get_node_as::<Label>("InfoLabel") {
            info_label.set_text("");
        }
        if let Some(mut dialog) = self.base_mut().try_get_node_as::<FileDialog>("FileDialog") {
            let window_size = self.base_mut().get_window().map(|w| w.get_size()).unwrap_or(Vector2i::new(1280, 720));
            dialog.set_size(Vector2i::new((window_size.x as f64 * 0.8) as i32, (window_size.y as f64 * 0.8) as i32));
            dialog.popup_centered();
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn FromDownloadsButton_Pressed(&mut self) {
        let downloads = Os::singleton().get_system_dir(SystemDir::DOWNLOADS);
        if let Some(mut info_label) = self.base_mut().try_get_node_as::<Label>("InfoLabel") {
            info_label.set_text(&format!("Loading from {}", downloads));
        }
        let files = std::fs::read_dir(downloads.to_string())
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.extension().map(|ext| ext == "psarc").unwrap_or(false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.convert_files(files);
    }

    #[func]
    #[allow(non_snake_case)]
    fn Dir_Selected(&mut self, dir: GString) {
        let files = std::fs::read_dir(dir.to_string())
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.extension().map(|ext| ext == "psarc").unwrap_or(false))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.convert_files(files);
    }

    #[func]
    #[allow(non_snake_case)]
    fn File_Selected(&mut self, path: GString) {
        self.convert_files(vec![PathBuf::from(path.to_string())]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn Files_Selected(&mut self, paths: PackedStringArray) {
        let files = paths
            .as_slice()
            .iter()
            .map(|p| PathBuf::from(p.to_string()))
            .collect::<Vec<_>>();
        self.convert_files(files);
    }

    fn convert_files(&mut self, files: Vec<PathBuf>) {
        if let Some(mut info_label) = self.base_mut().try_get_node_as::<Label>("InfoLabel") {
            let psarc_files: Vec<PathBuf> = files
                .into_iter()
                .filter(|path| path.extension().map(|ext| ext == "psarc").unwrap_or(false))
                .collect();
            if psarc_files.is_empty() {
                info_label.set_text("No valid .psarc files found");
                return;
            }
            let mut completed = 0;
            let mut failed = 0;
            for psarc in psarc_files {
                match SongRepository::global().lock().add_source(&psarc) {
                    Ok(_) => completed += 1,
                    Err(_) => failed += 1,
                }
            }
            info_label.set_text(&format!("Completed: {completed}, failed: {failed}"));
        }
    }

    #[func]
    #[allow(non_snake_case)]
    fn BackButton_Pressed(&mut self) {
        if let Some(tween) = &self.tween {
            tween.to_initial();
        }
        self.base_mut().emit_signal("closed", &[]);
    }

    pub fn animate_in(&self) {
        if let Some(tween) = &self.tween {
            tween.to_final();
        }
    }

    pub fn animate_out(&self) {
        if let Some(tween) = &self.tween {
            tween.to_initial();
        }
    }
}
