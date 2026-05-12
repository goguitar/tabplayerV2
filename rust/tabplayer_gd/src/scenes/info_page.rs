use super::shared::*;

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
pub struct InfoPage {
    #[base]
    base: Base<VBoxContainer>,
    tween: Option<TweenHelper>,
}

#[godot_api]
impl IVBoxContainer for InfoPage {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self { base, tween: None }
    }

    fn ready(&mut self) {
        let tween = {
            let mut base = self.base_mut();
            let initial_pos = Vector2::new(-500.0, base.get_position().y);
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
impl InfoPage {
    #[signal]
    fn closed();

    #[func]
    #[allow(non_snake_case)]
    fn BackButton_Pressed(&mut self) {
        self.base_mut().emit_signal("closed", &[]);
    }

    #[func]
    #[allow(non_snake_case)]
    fn ProjectSourceButton_Pressed(&mut self) {
        Os::singleton().shell_open("https://github.com/Murph9/tabplayerV2");
    }

    #[func]
    #[allow(non_snake_case)]
    fn OpenConfigFolder_Pressed(&mut self) {
        let folder = Os::singleton().get_user_data_dir();
        let uri = format!("file://{folder}");
        Os::singleton().shell_open(&uri);
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
