use egui::Color32;
use egui::Context;
use egui::Label;
use egui::RichText;
use egui::Ui;
use egui::containers::Frame;
use std::cell::RefCell;
use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;

use crate::app;
use crate::app::MyApp;
use crate::plugin::Plugin;
use crate::plugin::Token;
use crate::plugin::change_progress;
use crate::plugins::jotoba_plugin::jotoba_tokenizer::Furigana;
use crate::plugins::jotoba_plugin::jotoba_tokenizer::JotobaTokenizer;
use crate::plugins::jotoba_plugin::jotoba_tokenizer::PartOfSpeech;

pub struct JotobaPlugin {
    tokens: Vec<Token>,
    jotoba_tokenizer: RefCell<JotobaTokenizer>, // TODO: REMOVE THIS REFCELL WHEN POSSIBLE
}

impl Plugin for JotobaPlugin {
    fn load_plugin(sentence: &str, progress: Arc<Mutex<String>>) -> Self {
        let mut jotoba_tokenizer: JotobaTokenizer = JotobaTokenizer::new();
        change_progress(&progress, "Tokenizing...");
        match jotoba_tokenizer.tokenize(sentence) {
            Ok(tokens) => Self {
                tokens,
                jotoba_tokenizer: RefCell::from(jotoba_tokenizer),
            },
            Err(e) => {
                // TODO: Add proper error handling.
                tracing::error!("Failed to tokenize input text with Jotoba due to error: {e}");
                panic!("{e}");
            }
        }
    }

    fn get_tokens(&self) -> &Vec<Token> {
        &self.tokens
    }

    fn display_token(&self, ui: &mut Ui, frame: &Frame, app: &MyApp, token: &Token) {
        if token.is_valid() {
            match self.jotoba_tokenizer.borrow_mut().get_response(token) {
                Ok(response) => {
                    /*
                    egui::TopBottomPanel::bottom("jotoba_footer")
                        .show_separator_line(false)
                        .frame(*frame)
                        .show(ctx, |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button(RichText::new("Open").size(20.0)).clicked() {
                                    ctx.open_url(egui::output::OpenUrl {
                                        url: format!(
                                            "https://jotoba.de/search/0/{}?l=en-US",
                                            self.get_sentence_string()
                                        ),
                                        new_tab: true,
                                    });
                                }
                            });
                        });
                    */
                    /*
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| {*/
                    for word in &response.words {
                        if let Some(furigana) = &word.reading.furigana {
                            Self::display_furigana(ui, &furigana.furigana);
                        } else {
                            if let Some(kanji) = &word.reading.kanji {
                                tracing::warn!(
                                    "Kanji {} without furigana in Jotoba response.",
                                    kanji
                                );
                                ui.label(RichText::new(kanji).heading()); //.size(22.0));
                            } else {
                                ui.label(
                                    RichText::new(&word.reading.kana).heading(), //.size(22.0)
                                );
                            }
                        }

                        /*
                        let mut count: u32 = 1;
                        for sense in &word.senses {
                            ui.label(
                                RichText::new(format!("{}. {}", count, sense.glosses.join(", ")))
                                    .small(),
                            );
                            count += 1;
                        }*/

                        let mut count: u32 = 0;
                        let mut last_tags: Vec<PartOfSpeech> = Vec::new();
                        for sense in &word.senses {
                            let tags: Vec<PartOfSpeech> = match &sense.pos {
                                Some(tags) => tags.clone(),
                                None => Vec::new(),
                            };
                            if tags != last_tags {
                                last_tags = tags.clone();
                                if count > 0 {
                                    ui.add_space(app::SPACING_SIZE);
                                    count = 1;
                                }
                                //ui.add_space(app::SPACING_SIZE * 0.5);
                                Self::display_tags(ui, &tags);
                            }
                            if count == 0 {
                                count = 1;
                            }

                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new(format!("{}.", count))
                                        .small()
                                        .color(app::SECONDARY_TEXT_COLOR),
                                );
                                ui.label(
                                    RichText::new(format!("{}", sense.glosses.join(", "))).small(),
                                );
                            });

                            let mut info: Vec<String> = Vec::new();
                            if let Some(misc) = &sense.misc {
                                // Turn from CamelCase into regular sentence
                                let mut misc_readable = String::new();
                                for c in misc.chars() {
                                    if misc_readable.is_empty() {
                                        misc_readable.push(c);
                                    } else {
                                        if c.is_uppercase() {
                                            misc_readable.push(' ');
                                            misc_readable
                                                .push_str(&c.to_lowercase().collect::<String>());
                                        } else {
                                            misc_readable.push(c);
                                        }
                                    }
                                }
                                info.push(misc_readable);
                            }
                            if let Some(information) = &sense.information {
                                info.push(information.clone());
                            }
                            if let Some(xref) = &sense.xref {
                                // xref sometimes looks like "お手・おて・１"; no idea why. Just
                                // take the first element:
                                if let Some(first) = xref.split("・").nth(0) {
                                    info.push(format!("see also: {}", first));
                                }
                            }

                            if !info.is_empty() {
                                let info_string: String = info.join(", ");
                                ui.horizontal_top(|ui| {
                                    ui.add(
                                        Label::new(
                                            RichText::new(format!("{}.", count))
                                                .small()
                                                .color(Color32::TRANSPARENT),
                                        )
                                        .selectable(false),
                                    );
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new(format!("{}", info_string))
                                                .size(app::TINY_TEXT_SIZE * 0.9)
                                                .color(app::SECONDARY_TEXT_COLOR),
                                        );
                                    });
                                });
                            }

                            count += 1;
                        }

                        ui.add_space(app::SPACING_SIZE * 0.5);

                        let percent: f32 = 0.8;
                        let width: f32 = ui.available_width() * percent;
                        let margin: f32 = (ui.available_width() - width) / 2.0;

                        ui.horizontal(|ui| {
                            ui.add_space(margin);
                            let rect: egui::Rect = ui.allocate_space(egui::vec2(width, 1.0)).1;
                            ui.painter().line_segment(
                                [rect.left_center(), rect.right_center()],
                                egui::Stroke::new(
                                    1.0,
                                    Color32::from_rgba_premultiplied(20, 20, 20, 20),
                                ),
                            );
                        });

                        ui.add_space(app::SPACING_SIZE * 0.5);
                    }

                    /* });*/
                }
                Err(e) => tracing::debug!("Could not display token due to error: {e}"),
            };
        }
    }

    fn open(&self, ctx: &Context) {
        tracing::info!("Trying to open Jotoba website with input text.");

        match self.build_sanitized_url() {
            Ok(url) => {
                ctx.open_url(egui::OpenUrl::new_tab(url));
            }
            Err(e) => {
                tracing::warn!("Could not build Jotoba URL due to error: {}", e);
            }
        }
    }
}

impl JotobaPlugin {
    fn get_sentence_string(&self) -> String {
        self.tokens
            .iter()
            .map(|token| token.input_word.to_owned())
            .collect::<Vec<String>>()
            .join("")
    }

    fn build_sanitized_url(&self) -> Result<String, Box<dyn Error>> {
        let mut url =
            reqwest::Url::parse_with_params("https://jotoba.de/search/0", &[("l", "en-US")])
                .map_err(|e| e.to_string())?;

        url.path_segments_mut()
            .map_err(|_| "URL cannot be a base")?
            .push(&self.get_sentence_string());

        Ok(url.to_string())
    }

    fn display_tags(ui: &mut Ui, tags: &Vec<PartOfSpeech>) {
        ui.horizontal_wrapped(|ui| {
            //TODO: holy... process tags elsewhere, map to some kind of enum or something.
            for tag in tags {
                match tag {
                    PartOfSpeech::Simple(tag) => {
                        Self::display_tag(ui, tag, tag);
                    }
                    PartOfSpeech::Complex(tags) => {
                        for (tag, pos) in tags.iter() {
                            match pos {
                                PartOfSpeech::Simple(hint) => {
                                    Self::display_tag(ui, tag, hint);
                                }
                                PartOfSpeech::Complex(hints) => {
                                    for (hint, subhint) in hints {
                                        match subhint {
                                            PartOfSpeech::Simple(s) => {
                                                Self::display_tag(
                                                    ui,
                                                    tag,
                                                    &format!("{} ({})", hint, s),
                                                );
                                            }
                                            PartOfSpeech::Complex(c) => {
                                                if let Some(class) = c.get("class") {
                                                    match class {
                                                        PartOfSpeech::Simple(x) => {
                                                            Self::display_tag(
                                                                ui,
                                                                tag,
                                                                &format!("{} ({})", hint, x),
                                                            );
                                                        }
                                                        PartOfSpeech::Complex(y) => {
                                                            tracing::warn!(
                                                                "Unhandled tag for POS: {:?}",
                                                                tags
                                                            );
                                                        }
                                                    }
                                                } else {
                                                    tracing::warn!(
                                                        "Unhandled tag for POS: {:?}",
                                                        tags
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            };
                        }
                    }
                };
            }
        });
    }

    // TODO: Almost same exact function copied over from kihon_plugin. Perhaps unify?
    fn display_tag(ui: &mut Ui, tag: &str, hint: &str) {
        let text_galley = ui.fonts_mut(|f| {
            f.layout_no_wrap(
                tag.to_string(),
                egui::FontId::proportional(app::TINY_TEXT_SIZE),
                app::PRIMARY_TEXT_COLOR,
            )
        });

        let padding = egui::Vec2::new(4.0, 0.0);
        let rect = egui::Rect::from_min_size(ui.cursor().min, text_galley.size() + (2.0 * padding));
        let response = ui
            .allocate_rect(rect, egui::Sense::hover())
            .on_hover_text(RichText::new(hint).size(app::TINY_TEXT_SIZE));

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

    // TODO: Same exact function copied over from kihon_plugin. Perhaps unify?
    fn display_furigana(ui: &mut Ui, furigana_vec: &Vec<Furigana>) {
        let vertical_gap: f32 = 1.0;

        // calculate how wide (and tall) the entire string will be
        let mut total_width: f32 = 0.0;
        let mut max_height: f32 = 0.0;
        let mut galley_data = Vec::new();

        for furigana in furigana_vec {
            let main_galley = ui.fonts_mut(|f| {
                f.layout_no_wrap(
                    furigana.ruby.to_string(),
                    egui::FontId::proportional(app::BIG_TEXT_SIZE),
                    app::PRIMARY_TEXT_COLOR,
                )
            });

            let furigana_galley = if let Some(reading) = &furigana.rt {
                ui.fonts_mut(|f| {
                    f.layout_no_wrap(
                        reading.to_string(),
                        egui::FontId::proportional(app::TINY_TEXT_SIZE),
                        app::LIGHT_TEXT_COLOR,
                    )
                })
            } else {
                ui.fonts_mut(|f| {
                    f.layout_no_wrap(
                        "あ".to_string(), // invisible placeholder
                        egui::FontId::proportional(app::TINY_TEXT_SIZE),
                        Color32::TRANSPARENT,
                    )
                })
            };

            let char_width: f32 = main_galley.size().x.max(furigana_galley.size().x);
            let char_height: f32 = main_galley.size().y + furigana_galley.size().y + vertical_gap;

            total_width += char_width;
            max_height = max_height.max(char_height);

            galley_data.push((main_galley, furigana_galley, char_width));
        }

        // then draw without gap between galleys
        let (rect, _) = ui.allocate_exact_size(
            egui::Vec2::new(total_width, max_height),
            egui::Sense::empty(),
        );

        let mut current_x: f32 = rect.left();
        for (main_galley, furigana_galley, char_width) in galley_data {
            let furigana_pos = egui::Pos2::new(
                current_x + (char_width - furigana_galley.size().x) * 0.5,
                rect.top(),
            );
            ui.painter()
                .galley(furigana_pos, furigana_galley, Color32::PLACEHOLDER);

            let main_pos = egui::Pos2::new(
                current_x + (char_width - main_galley.size().x) * 0.5,
                rect.top() + app::TINY_TEXT_SIZE + vertical_gap,
            );
            ui.painter()
                .galley(main_pos, main_galley, Color32::PLACEHOLDER);

            current_x += char_width;
        }
    }
}
