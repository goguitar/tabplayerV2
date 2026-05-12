use super::shared::*;

#[derive(GodotClass)]
#[class(base=VBoxContainer)]
pub struct SettingsPage {
    #[base]
    base: Base<VBoxContainer>,
    settings: Settings,
    tween: Option<TweenHelper>,
    color_pickers: Vec<Gd<ColorPickerButton>>,
}

#[godot_api]
impl IVBoxContainer for SettingsPage {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self {
            base,
            settings: SettingsService::settings(),
            tween: None,
            color_pickers: Vec::new(),
        }
    }

    fn ready(&mut self) {
        self.settings = SettingsService::settings();
        self.color_pickers = Vec::new();
        self.build_ui();
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
impl SettingsPage {
    #[signal]
    fn closed();

    fn build_ui(&mut self) {
        let mut header = HBoxContainer::new_alloc();
        let mut title = Label::new_alloc();
        title.set_text("Settings");
        header.add_child(Some(&title.upcast::<Node>()));
        let mut exit_button = Button::new_alloc();
        exit_button.set_text("Save and Close");
        exit_button.connect("pressed", &self.base_mut().callable("on_close_pressed"));
        header.add_child(Some(&exit_button.upcast::<Node>()));
        self.base_mut().add_child(Some(&header.upcast::<Node>()));

        let mut label = Label::new_alloc();
        label.set_text("Set String Colours (low to high):");
        self.base_mut().add_child(Some(&label.upcast::<Node>()));

        for i in 0..6 {
            let string_char = DisplayConst::STRING_LABELS[i];
            let mut box_container = HBoxContainer::new_alloc();
            let mut string_label = Label::new_alloc();
            string_label.set_text(&format!("{string_char} String"));
            box_container.add_child(Some(&string_label.upcast::<Node>()));
            let mut picker = ColorPickerButton::new_alloc();
            picker.set_pick_color(self.settings.string_colours[i]);
            picker.set_edit_alpha(false);
            let args = [i.to_variant()];
            let callable = self.base_mut().callable("on_color_changed").bind(&args);
            picker.connect("popup_closed", &callable);
            self.color_pickers.push(picker.clone());
            box_container.add_child(Some(&picker.upcast::<Node>()));
            self.base_mut().add_child(Some(&box_container.upcast::<Node>()));
        }

        let mut other_label = Label::new_alloc();
        other_label.set_text("Other Settings");
        self.base_mut().add_child(Some(&other_label.upcast::<Node>()));

        let mut low_is_low = CheckBox::new_alloc();
        low_is_low.set_text("Low String at the bottom");
        low_is_low.set_pressed(self.settings.low_string_is_low);
        low_is_low.connect("pressed", &self.base_mut().callable("on_low_is_low_toggled"));
        self.base_mut().add_child(Some(&low_is_low.upcast::<Node>()));

        let mut camera_box = HBoxContainer::new_alloc();
        let mut camera_label = Label::new_alloc();
        camera_label.set_text(&format!("Change Camera Aim Speed: {}", self.settings.camera_aim_speed));
        camera_box.add_child(Some(&camera_label.upcast::<Node>()));
        let mut camera_up = Button::new_alloc();
        camera_up.set_text("(+)");
        camera_up.connect("pressed", &self.base_mut().callable("on_camera_speed_up"));
        camera_box.add_child(Some(&camera_up.upcast::<Node>()));
        let mut camera_down = Button::new_alloc();
        camera_down.set_text("(-)");
        camera_down.connect("pressed", &self.base_mut().callable("on_camera_speed_down"));
        camera_box.add_child(Some(&camera_down.upcast::<Node>()));
        self.base_mut().add_child(Some(&camera_box.upcast::<Node>()));

        let mut audio_box = HBoxContainer::new_alloc();
        let mut audio_label = Label::new_alloc();
        audio_label.set_text("AudioOffset (in Ms): ");
        audio_box.add_child(Some(&audio_label.upcast::<Node>()));
        let mut audio_offset = SpinBox::new_alloc();
        audio_offset.set_min(0.0);
        audio_offset.set_max(5000.0);
        audio_offset.set_step(1.0);
        audio_offset.set_value_no_signal(self.settings.audio_position_offset_ms);
        audio_offset.connect("value_changed", &self.base_mut().callable("on_audio_offset_changed"));
        audio_box.add_child(Some(&audio_offset.upcast::<Node>()));
        self.base_mut().add_child(Some(&audio_box.upcast::<Node>()));
    }

    #[func]
    fn on_close_pressed(&mut self) {
        self.base_mut().emit_signal("closed", &[]);
    }

    #[func]
    fn on_low_is_low_toggled(&mut self) {
        self.settings.low_string_is_low = !self.settings.low_string_is_low;
        SettingsService::update_settings(self.settings.clone());
    }

    #[func]
    fn on_camera_speed_up(&mut self) {
        self.adjust_camera_speed(1.0);
    }

    #[func]
    fn on_camera_speed_down(&mut self) {
        self.adjust_camera_speed(-1.0);
    }

    fn adjust_camera_speed(&mut self, delta: f32) {
        let new_speed = self.settings.camera_aim_speed + delta;
        if new_speed < 1.0 || new_speed > 30.0 {
            return;
        }
        self.settings.camera_aim_speed = new_speed;
        SettingsService::update_settings(self.settings.clone());
    }

    #[func]
    fn on_audio_offset_changed(&mut self, value: f64) {
        self.settings.audio_position_offset_ms = value;
        SettingsService::update_settings(self.settings.clone());
    }

    #[func]
    fn on_color_changed(&mut self, index: i64) {
        if let Some(picker) = self.color_pickers.get(index as usize) {
            let color = picker.get_pick_color();
            if let Some(entry) = self.settings.string_colours.get_mut(index as usize) {
                *entry = color;
                SettingsService::update_settings(self.settings.clone());
            }
        }
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
