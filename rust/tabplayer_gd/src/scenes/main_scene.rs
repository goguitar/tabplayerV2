use super::shared::*;
use super::{load_scene, ConvertMenu, InfoPage, SettingsPage, SongPick, SongScene, StartMenu};

#[derive(GodotClass)]
#[class(base=Node)]
pub struct MainScene {
    #[base]
    base: Base<Node>,
    start_menu: Option<Gd<StartMenu>>,
    song_pick: Option<Gd<SongPick>>,
    convert_menu: Option<Gd<ConvertMenu>>,
    info_page: Option<Gd<InfoPage>>,
    settings_page: Option<Gd<SettingsPage>>,
}

#[godot_api]
impl INode for MainScene {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            start_menu: None,
            song_pick: None,
            convert_menu: None,
            info_page: None,
            settings_page: None,
        }
    }

    fn ready(&mut self) {
        self.load_menus();
        self.load_song_pick();
    }
}

#[godot_api]
impl MainScene {
    fn load_menus(&mut self) {
        self.load_start_menu();

        let mut convert_menu = load_scene::<ConvertMenu>("res://scenes/ConvertMenu.tscn");
        self.base_mut().add_child(Some(&convert_menu.clone().upcast::<Node>()));
        convert_menu.connect(
            "closed",
            &self.base_mut().callable("on_convert_closed"),
        );
        self.convert_menu = Some(convert_menu);

        let mut info_page = load_scene::<InfoPage>("res://scenes/InfoPage.tscn");
        self.base_mut().add_child(Some(&info_page.clone().upcast::<Node>()));
        info_page.connect("closed", &self.base_mut().callable("on_info_closed"));
        self.info_page = Some(info_page);

        let mut settings_page = load_scene::<SettingsPage>("res://scenes/SettingsPage.tscn");
        self.base_mut()
            .add_child(Some(&settings_page.clone().upcast::<Node>()));
        settings_page.connect(
            "closed",
            &self.base_mut().callable("on_settings_closed"),
        );
        self.settings_page = Some(settings_page);
    }

    fn load_start_menu(&mut self) {
        let mut start_menu = load_scene::<StartMenu>("res://scenes/StartMenu.tscn");
        self.base_mut().add_child(Some(&start_menu.clone().upcast::<Node>()));
        start_menu.connect("closed", &self.base_mut().callable("on_start_closed"));
        start_menu.connect(
            "song_pick_opened",
            &self.base_mut().callable("on_song_pick_opened"),
        );
        start_menu.connect(
            "song_list_file_changed",
            &self.base_mut().callable("reload_song_list"),
        );
        start_menu.connect(
            "convert_menu_opened",
            &self.base_mut().callable("on_convert_opened"),
        );
        start_menu.connect(
            "info_menu_opened",
            &self.base_mut().callable("on_info_opened"),
        );
        start_menu.connect(
            "settings_opened",
            &self.base_mut().callable("on_settings_opened"),
        );
        self.start_menu = Some(start_menu);
    }

    #[func]
    fn on_start_closed(&mut self) {
        self.base_mut().get_tree().quit();
    }

    #[func]
    fn on_song_pick_opened(&mut self) {
        if let Some(mut start_menu) = self.start_menu.take() {
            self.base_mut().remove_child(Some(&start_menu.clone().upcast::<Node>()));
        }
        if let Some(mut convert_menu) = self.convert_menu.as_mut() {
            self.base_mut().remove_child(Some(&convert_menu.clone().upcast::<Node>()));
        }
        if let Some(mut settings_page) = self.settings_page.as_mut() {
            self.base_mut().remove_child(Some(&settings_page.clone().upcast::<Node>()));
        }
        if let Some(mut info_page) = self.info_page.as_mut() {
            self.base_mut().remove_child(Some(&info_page.clone().upcast::<Node>()));
        }
        if let Some(song_pick) = &self.song_pick {
            self.base_mut().add_child(Some(&song_pick.clone().upcast::<Node>()));
        }
    }

    #[func]
    fn on_convert_opened(&mut self) {
        if let Some(convert_menu) = &self.convert_menu {
            convert_menu.bind().animate_in();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_out();
        }
    }

    #[func]
    fn on_info_opened(&mut self) {
        if let Some(info_page) = &self.info_page {
            info_page.bind().animate_in();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_out();
        }
    }

    #[func]
    fn on_settings_opened(&mut self) {
        if let Some(settings_page) = &self.settings_page {
            settings_page.bind().animate_in();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_out();
        }
    }

    #[func]
    fn on_convert_closed(&mut self) {
        if let Some(convert_menu) = &self.convert_menu {
            convert_menu.bind().animate_out();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_in();
        }
        self.reload_song_list();
    }

    #[func]
    fn on_info_closed(&mut self) {
        if let Some(info_page) = &self.info_page {
            info_page.bind().animate_out();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_in();
        }
    }

    #[func]
    fn on_settings_closed(&mut self) {
        if let Some(settings_page) = &self.settings_page {
            settings_page.bind().animate_out();
        }
        if let Some(start_menu) = &self.start_menu {
            start_menu.bind().animate_in();
        }
    }

    #[func]
    fn reload_song_list(&mut self) {
        let loaded = self.song_pick.as_ref().map(|s| s.is_visible_in_tree()).unwrap_or(false);
        if loaded {
            if let Some(song_pick) = &self.song_pick {
                song_pick.queue_free();
            }
        }
        self.load_song_pick();
        if loaded {
            if let Some(song_pick) = &self.song_pick {
                self.base_mut().add_child(Some(&song_pick.clone().upcast::<Node>()));
            }
        }
    }

    fn load_song_pick(&mut self) {
        let mut song_pick = load_scene::<SongPick>("res://scenes/SongPick.tscn");
        song_pick.connect("closed", &self.base_mut().callable("on_song_pick_closed"));
        song_pick.connect("opened_song", &self.base_mut().callable("on_song_opened"));
        self.song_pick = Some(song_pick);
    }

    #[func]
    fn on_song_pick_closed(&mut self) {
        if let Some(song_pick) = &self.song_pick {
            self.base_mut().remove_child(Some(&song_pick.clone().upcast::<Node>()));
        }
        if let Some(start_menu) = &self.start_menu {
            self.base_mut().add_child(Some(&start_menu.clone().upcast::<Node>()));
            start_menu.bind().animate_in();
        }
        if let Some(convert_menu) = &self.convert_menu {
            self.base_mut().add_child(Some(&convert_menu.clone().upcast::<Node>()));
        }
        if let Some(settings_page) = &self.settings_page {
            self.base_mut().add_child(Some(&settings_page.clone().upcast::<Node>()));
        }
        if let Some(info_page) = &self.info_page {
            self.base_mut().add_child(Some(&info_page.clone().upcast::<Node>()));
        }
    }

    #[func]
    fn on_song_opened(&mut self, folder: GString, instrument: GString) {
        if let Some(song_pick) = &self.song_pick {
            self.base_mut().remove_child(Some(&song_pick.clone().upcast::<Node>()));
        }
        let mut scene = load_scene::<SongScene>("res://scenes/SongScene.tscn");
        let state = SongRepository::global()
            .lock()
            .get_song_state(&folder.to_string(), &instrument.to_string());
        if let Some(state) = state {
            scene.bind_mut().init(state);
        }
        self.base_mut().add_child(Some(&scene.clone().upcast::<Node>()));
        scene.connect("closed", &self.base_mut().callable("on_song_scene_closed"));
    }

    #[func]
    fn on_song_scene_closed(&mut self) {
        if let Some(song_pick) = &self.song_pick {
            self.base_mut().add_child(Some(&song_pick.clone().upcast::<Node>()));
        }
    }
}
