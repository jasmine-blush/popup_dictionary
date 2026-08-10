use egui::Color32;
use egui::Context;
use egui::RichText;
use egui::Ui;
use egui::containers::Frame;
use std::error::Error;
use std::sync::{Arc, Mutex};

use crate::app;
use crate::app::MyApp;
use crate::plugin::Plugin;
use crate::plugin::Token;
use crate::plugin::Validity;
use crate::plugin::change_progress;
use crate::plugins::mozhi_plugin::mozhi_translator::Translation;

pub struct MozhiPlugin {
    tokens: Vec<Token>,
    translations: Vec<Translation>,
}

impl Plugin for MozhiPlugin {
    fn load_plugin(sentence: &str, progress: Arc<Mutex<String>>) -> Self {
        change_progress(&progress, "Translating...");
        match crate::plugins::mozhi_plugin::mozhi_translator::translate(sentence) {
            Ok(translations) => Self {
                tokens: vec![Token {
                    input_word: sentence.to_owned(),
                    deinflected_word: sentence.to_owned(),
                    conjugations: Vec::new(),
                    validity: Validity::VALID,
                }],
                translations,
            },
            Err(e) => {
                // TODO: Add proper error handling.
                tracing::error!("Failed to translate input text with Mozhi due to error: {e}");
                panic!("{e}");
            }
        }
    }

    fn get_tokens(&self) -> &Vec<Token> {
        &self.tokens
    }

    fn display_token(&self, ui: &mut Ui, frame: &Frame, app: &MyApp, token: &Token) {
        for translation in &self.translations {
            ui.add_space(app::SPACING_SIZE);

            Self::display_tag(
                ui,
                &translation.engine,
                "Response from this Translation Engine",
            );
            ui.add_space(app::SPACING_SIZE * 0.5);
            ui.label(RichText::new(translation.translation.clone()).small());

            ui.add_space(app::SPACING_SIZE * 0.5);

            let percent: f32 = 0.8;
            let width: f32 = ui.available_width() * percent;
            let margin: f32 = (ui.available_width() - width) / 2.0;

            ui.horizontal(|ui| {
                ui.add_space(margin);
                let rect: egui::Rect = ui.allocate_space(egui::vec2(width, 1.0)).1;
                ui.painter().line_segment(
                    [rect.left_center(), rect.right_center()],
                    egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(20, 20, 20, 20)),
                );
            });

            ui.add_space(app::SPACING_SIZE * 0.5);
        }
    }

    fn open(&self, ctx: &Context) {
        tracing::info!("Trying to open Mozhi website with input text.");

        match self.build_sanitized_url() {
            Ok(url) => {
                ctx.open_url(egui::OpenUrl::new_tab(url));
            }
            Err(e) => {
                tracing::warn!("Could not build Mozhi URL due to error: {}", e);
            }
        }
    }
}

impl MozhiPlugin {
    fn build_sanitized_url(&self) -> Result<String, Box<dyn Error>> {
        let mut url = reqwest::Url::parse_with_params(
            "https://translate.projectsegfau.lt/",
            &[
                ("engine", "all"),
                ("from", "ja"),
                ("to", "en"),
                ("text", &self.tokens[0].input_word),
            ],
        )
        .map_err(|e| e.to_string())?;

        Ok(url.to_string())
    }

    // TODO: Almost same exact function copied over from kihon_plugin. Perhaps unify?
    fn display_tag(ui: &mut Ui, tag: &str, hint: &str) {
        let text_galley = ui.fonts_mut(|f| {
            f.layout_no_wrap(
                tag.to_string(),
                egui::FontId::proportional(app::SMALL_TEXT_SIZE),
                app::PRIMARY_TEXT_COLOR,
            )
        });

        let padding = egui::Vec2::new(4.0, 0.0);
        let rect = egui::Rect::from_min_size(ui.cursor().min, text_galley.size() + (2.0 * padding));
        let response = ui
            .allocate_rect(rect, egui::Sense::hover())
            .on_hover_text(RichText::new(hint).size(app::SMALL_TEXT_SIZE));

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Help);
        }

        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(app::CORNER_RADIUS),
            app::SECONDARY_BACKGROUND_COLOR,
        );

        ui.painter().galley(
            (rect.center() - text_galley.size() / 2.0) - egui::Vec2::new(0.0, 2.0),
            text_galley,
            app::PRIMARY_TEXT_COLOR,
        );

        //ui.allocate_space(rect.size());
    }
}
