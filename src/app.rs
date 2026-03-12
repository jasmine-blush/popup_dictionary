#[cfg(feature = "hyprland-support")]
use hyprland::ctl::plugin;

use eframe::{
    NativeOptions, egui,
    epaint::text::{FontInsert, InsertFontFamily},
};
use egui::{Color32, Context, CornerRadius, Pos2, Rect, RichText};
use std::sync::{Arc, Mutex};

use crate::plugin::{Plugin, Plugins, Token};

//pub const WINDOW_INIT_WIDTH: i16 = 450;
//pub const WINDOW_INIT_HEIGHT: i16 = 450;
pub const APP_NAME: &str = "Popup Dictionary";

const PRIMARY_BACKGROUND_COLOR: Color32 = Color32::from_rgb(30, 30, 30);
pub const SECONDARY_BACKGROUND_COLOR: Color32 = Color32::from_rgb(50, 50, 50);
pub const PRIMARY_TEXT_COLOR: Color32 = Color32::WHITE;
pub const SECONDARY_TEXT_COLOR: Color32 = Color32::GRAY;
pub const LIGHT_TEXT_COLOR: Color32 = Color32::LIGHT_GRAY;
pub const BIG_TEXT_SIZE: f32 = 24.0;
const PRIMARY_TEXT_SIZE: f32 = 20.0;
pub const SMALL_TEXT_SIZE: f32 = 18.0;
pub const TINY_TEXT_SIZE: f32 = 14.0;
pub const SPACING_SIZE: f32 = 10.0;
pub const CORNER_RADIUS: u8 = 4;

#[derive(Clone)]
pub struct Config {
    pub initial_plugin: Option<String>,
    pub open_at_cursor: bool,
    pub wrapped: bool,
    pub initial_width: u16,
    pub initial_height: u16,
    pub show_tray_icon: bool,
}

pub fn run_app(sentence: &str, config: Config) -> Result<(), eframe::Error> {
    #[cfg(feature = "hyprland-support")]
    let is_hyprland: bool = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok();
    #[cfg(feature = "hyprland-support")]
    tracing::debug!("Looks like Hyprland? {}.", is_hyprland);

    let mut init_pos: Option<Pos2> = None;
    let options: NativeOptions;
    if config.open_at_cursor {
        tracing::info!("Trying to open window at cursor position.");

        match crate::window_helper::get_optimal_init_pos(
            #[cfg(feature = "hyprland-support")]
            is_hyprland,
            config.initial_width as f32,
            config.initial_height as f32,
        ) {
            Ok(optimal_pos) => {
                init_pos = Some(optimal_pos);

                tracing::debug!(
                    "Found optimal window position at x: {}, y: {}.",
                    optimal_pos.x,
                    optimal_pos.y
                );

                options = NativeOptions {
                    viewport: egui::ViewportBuilder::default()
                        .with_position(optimal_pos)
                        .with_inner_size([
                            config.initial_width as f32,
                            config.initial_height as f32,
                        ])
                        .with_min_inner_size([100.0, 100.0])
                        .with_title(APP_NAME)
                        .with_active(true),
                    ..Default::default()
                };
            }
            Err(e) => {
                tracing::warn!("Could not get optimal window position due to error: {e}");

                options = NativeOptions {
                    viewport: egui::ViewportBuilder::default()
                        .with_inner_size([
                            config.initial_width as f32,
                            config.initial_height as f32,
                        ])
                        .with_min_inner_size([100.0, 100.0])
                        .with_title(APP_NAME)
                        .with_active(true),
                    ..Default::default()
                };
            }
        }
    } else {
        options = NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([config.initial_width as f32, config.initial_height as f32])
                .with_min_inner_size([100.0, 100.0])
                .with_title(APP_NAME)
                .with_active(true),
            ..Default::default()
        };
    }

    tracing::debug!(
        "Window config is width: {}, height: {}, name: {}, plugin: {}, wrapped: {}.",
        config.initial_width,
        config.initial_height,
        APP_NAME,
        config
            .initial_plugin
            .to_owned()
            .unwrap_or(String::from("None")),
        config.wrapped
    );
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| {
            Ok(Box::new(MyApp::new(
                cc,
                config,
                init_pos,
                #[cfg(feature = "hyprland-support")]
                is_hyprland,
                sentence,
            )))
        }),
    )
}

enum PluginState {
    Initial,
    Loading,
    Ready(Box<dyn Plugin>),
}

pub struct MyApp {
    config: Config,
    init_pos: Option<Pos2>,
    #[cfg(feature = "hyprland-support")]
    is_hyprland: bool,
    sentence: String,
    selected_token_index: Option<usize>,
    plugin_state: Arc<Mutex<PluginState>>,
    available_plugins: Vec<Plugins>,
    active_plugin_index: usize,
    theme_is_set: bool,
    main_frame: Option<egui::containers::Frame>,
}

impl MyApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        config: Config,
        init_pos: Option<Pos2>,
        #[cfg(feature = "hyprland-support")] is_hyprland: bool,
        sentence: &str,
    ) -> Self {
        Self::load_main_font(&cc.egui_ctx);

        let available_plugins: Vec<Plugins> = Plugins::all();

        let init_plugin_idx: usize = if let Some(init_plugin) = &config.initial_plugin {
            available_plugins
                .iter()
                .position(|p| p.name() == init_plugin)
                .unwrap_or(0)
        } else {
            0
        };

        let mut app = Self {
            config,
            init_pos,
            #[cfg(feature = "hyprland-support")]
            is_hyprland,
            sentence: sentence.to_string(),
            selected_token_index: None,
            plugin_state: Arc::new(Mutex::new(PluginState::Initial)),
            available_plugins,
            active_plugin_index: init_plugin_idx,
            theme_is_set: false,
            main_frame: None,
        };

        app.try_load_plugin(init_plugin_idx);

        tracing::info!("Opening the window.");
        app
    }

    fn load_main_font(ctx: &Context) {
        ctx.add_font(FontInsert::new(
            "NotoSansCJKJP",
            #[cfg(not(target_os = "windows"))]
            egui::FontData::from_static(include_bytes!("./assets/popup_font.ttc")), // Currently: Noto Sans CJK JP
            #[cfg(target_os = "windows")]
            egui::FontData::from_static(include_bytes!(".\\assets\\popup_font.ttc")), // Currently: Noto Sans CJK JP
            vec![
                InsertFontFamily {
                    family: egui::FontFamily::Proportional,
                    priority: egui::epaint::text::FontPriority::Highest,
                },
                InsertFontFamily {
                    family: egui::FontFamily::Monospace,
                    priority: egui::epaint::text::FontPriority::Lowest,
                },
            ],
        ));
    }

    fn try_load_plugin(&mut self, plugin_index: usize) {
        tracing::info!(
            "Trying to load plugin: {}",
            self.available_plugins.get(plugin_index).map_or(
                format!("{}?", plugin_index),
                |plugin| format!("{}.", plugin.name())
            )
        );

        if plugin_index >= self.available_plugins.len() {
            return;
        }

        let state_clone: Arc<Mutex<PluginState>> = Arc::clone(&self.plugin_state);
        {
            let mut state = state_clone.lock().unwrap();
            match *state {
                PluginState::Loading => {
                    tracing::info!("A plugin is currently loading.");
                    return;
                }
                PluginState::Ready(_) => {
                    if self.active_plugin_index == plugin_index {
                        tracing::info!("The same plugin is already loaded.");
                        return;
                    }
                    *state = PluginState::Loading;
                }
                _ => {
                    *state = PluginState::Loading;
                }
            }
        }

        let active_plugin: Plugins = self.available_plugins[plugin_index];
        let plugin_sentence: String = self.sentence.to_owned();
        std::thread::spawn(move || {
            // TODO: Implement error handling, logging?
            let plugin: Box<dyn Plugin> = active_plugin.generate(&plugin_sentence);
            *state_clone.lock().unwrap() = PluginState::Ready(plugin);
        });

        self.selected_token_index = None;
        self.active_plugin_index = plugin_index;
    }

    fn set_theme(&mut self, ctx: &Context) {
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(PRIMARY_TEXT_COLOR);
        visuals.window_fill = PRIMARY_BACKGROUND_COLOR;
        visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
        visuals.window_corner_radius = CornerRadius::ZERO;
        visuals.window_shadow = egui::Shadow::NONE;
        visuals.window_stroke = egui::Stroke::NONE;
        visuals.indent_has_left_vline = false;
        ctx.set_visuals(visuals);

        let mut spacing = ctx.style().spacing.clone();
        spacing.button_padding = egui::vec2(2.0, 2.0);
        spacing.icon_spacing = 2.0;
        spacing.item_spacing = egui::vec2(4.0, 4.0);
        spacing.indent = 4.0;
        spacing.menu_spacing = 2.0;
        spacing.window_margin = egui::Margin {
            left: 2,
            right: 2,
            top: 2,
            bottom: 2,
        };
        ctx.style_mut(|style| style.spacing = spacing);

        let mut style = (*ctx.style()).clone();
        style.text_styles = [
            (
                egui::TextStyle::Heading,
                egui::FontId::new(BIG_TEXT_SIZE, egui::FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Body,
                egui::FontId::new(PRIMARY_TEXT_SIZE, egui::FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Button,
                egui::FontId::new(PRIMARY_TEXT_SIZE, egui::FontFamily::Proportional),
            ),
            (
                egui::TextStyle::Small,
                egui::FontId::new(SMALL_TEXT_SIZE, egui::FontFamily::Proportional),
            ),
        ]
        .into();
        ctx.set_style(style);

        self.main_frame = Some(egui::containers::Frame {
            corner_radius: CornerRadius::ZERO,
            fill: PRIMARY_BACKGROUND_COLOR,
            inner_margin: egui::Margin {
                left: 2,
                right: 2,
                top: 2,
                bottom: 2,
            },
            outer_margin: egui::Margin {
                left: 2,
                right: 2,
                top: 2,
                bottom: 2,
            },
            shadow: egui::Shadow::NONE,
            stroke: egui::Stroke::NONE,
        });
    }

    fn display_token_header(
        ui: &mut egui::Ui,
        tokens: &Vec<Token>,
        selected_token_idx: usize,
    ) -> Option<usize> {
        for (idx, token) in tokens.iter().enumerate() {
            let mut label_text: RichText = RichText::new(&token.input_word).size(PRIMARY_TEXT_SIZE);
            if token.is_valid() {
                label_text = label_text.underline();
                if idx != selected_token_idx {
                    label_text = label_text.color(SECONDARY_TEXT_COLOR);
                }

                let text_size: egui::Vec2 = {
                    let temp_galley: Arc<egui::Galley> = ui.fonts_mut(|f| {
                        f.layout_no_wrap(
                            label_text.text().to_string(),
                            egui::FontId::proportional(PRIMARY_TEXT_SIZE),
                            Color32::PLACEHOLDER,
                        )
                    });
                    temp_galley.size()
                };
                let (background_rect, _) = ui.allocate_exact_size(text_size, egui::Sense::hover());
                let label_rect: Rect = Rect::from_center_size(background_rect.center(), text_size);

                let response = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(label_rect), |ui| {
                        ui.label(label_text)
                    })
                    .inner;
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    ui.painter().rect_filled(
                        background_rect,
                        CornerRadius::same(CORNER_RADIUS),
                        Color32::from_rgba_premultiplied(
                            SECONDARY_BACKGROUND_COLOR.r(),
                            SECONDARY_BACKGROUND_COLOR.g(),
                            SECONDARY_BACKGROUND_COLOR.b(),
                            40,
                        ),
                    );
                }
                if response.clicked() {
                    return Some(idx);
                }
            } else {
                ui.label(label_text);
            }
        }
        return None;
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if let Some(init_pos) = self.init_pos {
            #[cfg(feature = "hyprland-support")]
            if self.is_hyprland {
                if let Err(e) =
                    crate::window_helper::move_window_hyprland(init_pos.x as i16, init_pos.y as i16)
                {
                    tracing::warn!(
                        "Could not set initial window position on hyprland due to error: {e}"
                    );
                } else {
                    self.init_pos = None;
                }
            }

            #[cfg(not(feature = "wayland-support"))]
            if let Err(e) =
                crate::window_helper::move_window_x11(init_pos.x as i32, init_pos.y as i32)
            {
                tracing::warn!("Could not set initial window position on x11 due to error: {e}");
            } else {
                self.init_pos = None;
            }
        }

        if !self.theme_is_set {
            self.set_theme(ctx);
            self.theme_is_set = true;

            // theme_is_set basically acts like "just on first frame".
            // set window to focused on first frame (Windows)
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        let main_frame = self.main_frame.unwrap();

        egui::CentralPanel::default()
            .frame(main_frame)
            .show(ctx, |ui| {
                let footer_height = 42.0;

                match &(*self.plugin_state.lock().unwrap()) {
                    PluginState::Ready(plugin) => {
                        let tokens: &Vec<Token> = plugin.get_tokens();

                        if self.selected_token_index.is_none() {
                            let mut first_valid_idx: usize = 0;
                            let mut curr_idx: usize = 0;
                            while curr_idx < tokens.len() {
                                if tokens[curr_idx].is_valid() {
                                    first_valid_idx = curr_idx;
                                    break;
                                }
                                curr_idx += 1;
                            }
                            self.selected_token_index = Some(first_valid_idx);
                        }
                        let selected_token_idx: usize = self.selected_token_index.unwrap();

                        egui::ScrollArea::horizontal()
                            .id_salt("token_header")
                            .show(ui, |ui| {
                                if self.config.wrapped {
                                    ui.horizontal_wrapped(|ui| {
                                        if let Some(idx) = Self::display_token_header(
                                            ui,
                                            tokens,
                                            selected_token_idx,
                                        ) {
                                            self.selected_token_index = Some(idx);
                                        }
                                    });
                                } else {
                                    ui.horizontal(|ui| {
                                        if let Some(idx) = Self::display_token_header(
                                            ui,
                                            tokens,
                                            selected_token_idx,
                                        ) {
                                            self.selected_token_index = Some(idx);
                                        }
                                    });
                                }

                                ui.add_space(SPACING_SIZE);
                            });

                        ui.separator();

                        let center_height = ui.available_height() - footer_height;
                        egui::ScrollArea::vertical()
                            .id_salt("plugin_display_section")
                            .max_height(center_height)
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                plugin.display_token(
                                    ctx,
                                    &main_frame,
                                    self,
                                    ui,
                                    &tokens[selected_token_idx],
                                );
                            });
                    }
                    _ => {
                        let center_height = ui.available_height() - footer_height;
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), center_height),
                            egui::Layout::centered_and_justified(egui::Direction::TopDown),
                            |ui| {
                                ui.horizontal(|ui| {
                                    // horizontal centering by ms-eevee on github:
                                    //
                                    // We create a closure function containing our elements, as we will be calling it twice.
                                    // Any additional elements to be centered would go within this closure.
                                    let elements = |ui: &mut egui::Ui| {
                                        ui.spinner();
                                        ui.add(egui::Label::new(RichText::new(
                                            "Loading Plugin...",
                                        )));
                                    };

                                    // Create a new child Ui with the invisible flag set so that the element does not actually
                                    // get shown on the GUI.
                                    // As a sidenote, we are taking advantage of the fact that new_child() does not allocate any of
                                    // the widget's space in the parent UI, so we are free to draw as much as we want without
                                    // advancing the parent's cursor.
                                    let mut hidden =
                                        ui.new_child(egui::UiBuilder::new().invisible());

                                    // Call our elements closure, passing in the invisible Ui child to be rendered.
                                    elements(&mut hidden);

                                    // We get the size of the rendered elements through min_rect() here as well.
                                    let diff: f32 = hidden.min_rect().width();

                                    // Add a space before rendering the element to the main screen.
                                    ui.add_space((ui.max_rect().width() / 2.0) - (diff / 2.0));
                                    // Finally, render the elements to the main UI.
                                    elements(ui);
                                });
                            },
                        );
                        //ctx.request_repaint();
                    }
                }

                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), footer_height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        // Calculate right-side bar width
                        let button_width: f32 = SPACING_SIZE * 2.0;
                        let button_spacing: f32 = ui.spacing().item_spacing.x;
                        let num_buttons: f32 = 3.0;
                        let fixed_area_width = (button_width * num_buttons as f32)
                            + (button_spacing * (num_buttons - 1.0))
                            + button_spacing * 2.0;

                        let available_width: f32 = ui.available_width() - fixed_area_width;
                        egui::ScrollArea::horizontal()
                            .id_salt("plugin_footer")
                            .max_height(footer_height)
                            .max_width(available_width)
                            .show(ui, |ui| {
                                ui.horizontal_centered(|ui| {
                                    let mut clicked_idx: Option<usize> = None;
                                    for (idx, active_plugin) in
                                        self.available_plugins.iter().enumerate()
                                    {
                                        if ui
                                            .add(egui::Button::selectable(
                                                self.active_plugin_index == idx,
                                                RichText::new(active_plugin.name()),
                                            ))
                                            .clicked()
                                        {
                                            clicked_idx = Some(idx);
                                        }
                                    }
                                    if let Some(idx) = clicked_idx {
                                        if self.active_plugin_index != idx {
                                            self.try_load_plugin(idx);
                                        }
                                    }
                                });
                            });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(egui::Button::new(RichText::new("⚙").size(SMALL_TEXT_SIZE)))
                                .clicked()
                            {
                                // Settings button
                            }
                            if ui
                                .add(egui::Button::new(
                                    RichText::new("\u{1f4cb}").size(SMALL_TEXT_SIZE),
                                ))
                                .clicked()
                            {
                                // Copy button
                                let sentence: String = self.sentence.to_owned();
                                std::thread::spawn(|| {
                                    tracing::debug!("Trying to copy input text to clipboard.");
                                    let mut clipboard: arboard::Clipboard =
                                        arboard::Clipboard::new().unwrap();
                                    clipboard.set_text(sentence).unwrap();
                                    std::thread::sleep(std::time::Duration::from_secs(1));
                                    drop(clipboard); // since clipboard is dropped here, linux users need a clipboard manager to retain data
                                    tracing::debug!("Successfully copied input text to clipboard.");
                                });
                            }
                            if ui
                                .add(egui::Button::new(RichText::new("ℹ").size(SMALL_TEXT_SIZE)))
                                .clicked()
                            {
                                // Special button
                                if let PluginState::Ready(plugin) =
                                    &(*self.plugin_state.lock().unwrap())
                                {
                                    plugin.open(ctx);
                                }
                            }
                        });
                    },
                );
            });
    }
}
