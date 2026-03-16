use egui::Color32;
use egui::RichText;
use egui::Ui;
use std::error::Error;
use std::path::PathBuf;

use crate::app;
use crate::app::MyApp;
use crate::app::SPACING_SIZE;
use crate::plugin::Plugin;
use crate::plugin::Token;
use crate::plugins::kihon_plugin::jmdict_dictionary::{
    Dictionary, DictionaryEntry, DictionaryTerm, Furigana,
};
use crate::plugins::kihon_plugin::jumandic_tokenizer::tokenize;

const ATTRIBUTIONS_URL: &str =
    "https://github.com/jasmine-blush/popup_dictionary?tab=readme-ov-file#licensing--attributions";

pub struct KihonPlugin {
    tokens: Vec<Token>,
    dictionary: Dictionary,
}

impl Plugin for KihonPlugin {
    fn load_plugin(sentence: &str) -> Self {
        let result: Result<Self, Box<dyn Error>> = (|| {
            let db_path: PathBuf = match dirs::data_dir() {
                Some(path) => path.join("popup_dictionary").join("db"),
                None => {
                    return Err(Box::from(
                        "No valid data path found in environment variables.",
                    ));
                }
            };

            let dictionary = Dictionary::load_dictionary(&db_path)?;

            let tokens = tokenize(&sentence.to_string(), &dictionary)?;

            Ok(Self { tokens, dictionary })
        })();

        match result {
            Ok(plugin) => plugin,
            Err(e) => {
                // TODO: Add proper error handling.
                tracing::error!("Failed to tokenize input text with Kihon due to error: {e}");
                panic!("{e}");
            }
        }
    }

    fn get_tokens(&self) -> &Vec<Token> {
        &self.tokens
    }

    fn display_token(
        &self,
        ctx: &egui::Context,
        frame: &egui::containers::Frame,
        app: &MyApp,
        ui: &mut Ui,
        token: &Token,
    ) {
        let forms_string: String = token
            .conjugations
            .iter()
            .map(|form| crate::plugins::kihon_plugin::jumandic_tokenizer::get_form(form))
            .collect::<Vec<&str>>()
            .join(", ");
        if forms_string != "*" {
            /*
            ui.scope(|ui| {
                ui.style_mut()
                    .visuals
                    .widgets
                    .noninteractive
                    .bg_stroke
                    .color = Color32::from_rgba_premultiplied(10, 10, 10, 10);
                ui.separator();
            });*/
            ui.label(RichText::new(format!("Forms: {}", forms_string)).size(app::TINY_TEXT_SIZE));
        } else {
            ui.add_space((app::TINY_TEXT_SIZE) + app::SPACING_SIZE + 1.0);
        }
        ui.scope(|ui| {
            ui.style_mut()
                .visuals
                .widgets
                .noninteractive
                .bg_stroke
                .color = Color32::from_rgba_premultiplied(10, 10, 10, 10);
            ui.separator();
        });

        ui.indent("scroll_indent", |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    /*
                    Lookup in database in this order until exists:
                    1. base                                     -- first
                    2. surface
                    3. base minus last letter (e.g. 素敵な)
                    4. surface minus last letter                -- last
                    */
                    if let Some(dictionary_entry) = self
                        .dictionary
                        .lookup(&token.deinflected_word)
                        .expect(&format!(
                            "Error getting from database when looking up base: {}",
                            &token.deinflected_word
                        ))
                    {
                        self.display_terms_prioritized(ui, token, &dictionary_entry);
                    } else if let Some(dictionary_entry) =
                        self.dictionary.lookup(&token.input_word).expect(&format!(
                            "Error getting from database when looking up surface: {}",
                            &token.input_word
                        ))
                    {
                        self.display_terms_prioritized(ui, token, &dictionary_entry);
                    } else {
                        let mut base_minus_one: String = token.deinflected_word.clone();
                        _ = base_minus_one.pop();
                        if let Some(dictionary_entry) =
                            self.dictionary.lookup(&base_minus_one).expect(&format!(
                                "Error getting from database when looking up base-1: {}",
                                &base_minus_one
                            ))
                        {
                            self.display_terms_prioritized(ui, token, &dictionary_entry);
                        } else {
                            let mut surface_minus_one: String = token.input_word.clone();
                            _ = surface_minus_one.pop();
                            if let Some(dictionary_entry) =
                                self.dictionary.lookup(&surface_minus_one).expect(&format!(
                                    "Error getting from database when looking up surface-1: {}",
                                    &surface_minus_one
                                ))
                            {
                                self.display_terms_prioritized(ui, token, &dictionary_entry);
                            }
                        }
                    }

                    //ui.add_space(app::SPACING_SIZE * 4.0);
                });
        });
    }

    fn open(&self, ctx: &egui::Context) {
        tracing::info!(
            "Trying to open attributions for the Kihon plugin. If this does not work, go to: {}.",
            ATTRIBUTIONS_URL
        );

        ctx.open_url(egui::output::OpenUrl {
            url: String::from(ATTRIBUTIONS_URL),
            new_tab: true,
        });
    }
}

impl KihonPlugin {
    fn display_terms_prioritized(&self, ui: &mut Ui, token: &Token, entry: &DictionaryEntry) {
        /*
        Display terms in this priority:
        1. no kanji, same as surface        -- first
        2. no kanji, same as base
        3. has kanji, same as surface
        4. has kanji, same as base
        5. rest                             -- last
        */

        let mut terms_to_display: Vec<DictionaryTerm> = entry.terms.clone();
        let mut filtered_terms: Vec<DictionaryTerm> = Vec::new();
        terms_to_display.retain_mut(|term| {
            if term.term.is_empty() && term.reading == token.input_word {
                filtered_terms.push(term.clone());
                false
            } else {
                true
            }
        });
        Self::display_terms(ui, &filtered_terms);
        filtered_terms.clear();
        terms_to_display.retain_mut(|term| {
            if term.term.is_empty() && term.reading == token.deinflected_word {
                filtered_terms.push(term.clone());
                false
            } else {
                true
            }
        });
        Self::display_terms(ui, &filtered_terms);
        filtered_terms.clear();
        terms_to_display.retain_mut(|term| {
            if !term.term.is_empty() && term.reading == token.input_word {
                filtered_terms.push(term.clone());
                false
            } else {
                true
            }
        });
        Self::display_terms(ui, &filtered_terms);
        filtered_terms.clear();
        terms_to_display.retain_mut(|term| {
            if !term.term.is_empty() && term.reading == token.deinflected_word {
                filtered_terms.push(term.clone());
                false
            } else {
                true
            }
        });
        Self::display_terms(ui, &filtered_terms);
        Self::display_terms(ui, &terms_to_display);
    }

    fn display_terms(ui: &mut Ui, terms: &Vec<DictionaryTerm>) {
        for dictionary_term in terms {
            ui.horizontal(|ui| {
                if !dictionary_term.term.is_empty() {
                    if let Some(furigana_vec) = &dictionary_term.furigana {
                        Self::display_furigana(ui, furigana_vec);
                    } else {
                        let furigana: Vec<Furigana> = vec![Furigana {
                            ruby: dictionary_term.term.to_string(),
                            rt: Some(dictionary_term.reading.to_string()),
                        }];
                        Self::display_furigana(ui, &furigana);
                    }
                } else {
                    ui.label(RichText::new(&dictionary_term.reading).heading());
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(app::SPACING_SIZE);

                    if ui
                        .add(egui::Button::new(
                            RichText::new("\u{1f4cb}").size(app::TINY_TEXT_SIZE),
                        ))
                        .on_hover_text(
                            RichText::new("Copy term to clipboard").size(app::TINY_TEXT_SIZE),
                        )
                        .clicked()
                    {
                        // Copy button
                        let term: String = if !dictionary_term.term.is_empty() {
                            dictionary_term.term.to_owned()
                        } else {
                            dictionary_term.reading.to_owned()
                        };
                        std::thread::spawn(|| {
                            tracing::debug!("Trying to copy term to clipboard.");
                            let mut clipboard: arboard::Clipboard =
                                arboard::Clipboard::new().unwrap();
                            clipboard.set_text(term).unwrap();
                            std::thread::sleep(std::time::Duration::from_secs(1));
                            drop(clipboard); // since clipboard is dropped here, linux users need a clipboard manager to retain data
                            tracing::debug!("Successfully copied term to clipboard.");
                        });
                    }
                });
            });

            let mut count: u32 = 0;
            let mut last_tags: String = String::new();
            for meaning in &dictionary_term.meanings {
                let tags: String = meaning.tags.join("");
                if tags != last_tags {
                    last_tags = tags.clone();
                    if count > 0 {
                        ui.add_space(app::SPACING_SIZE);
                        count = 1;
                    }
                    //ui.add_space(app::SPACING_SIZE * 0.5);
                    Self::display_tags(ui, &meaning.tags);
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
                    ui.label(RichText::new(format!("{}", meaning.gloss.join(", "))).small());
                });
                if meaning.info.len() > 0 {
                    ui.horizontal_top(|ui| {
                        ui.label(
                            RichText::new(format!("{}.", count))
                                .small()
                                .color(Color32::TRANSPARENT),
                        );
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(format!("{}", meaning.info.join("; ")))
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
                    egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(20, 20, 20, 20)),
                );
            });

            ui.add_space(SPACING_SIZE * 0.5);
        }
    }

    fn display_tags(ui: &mut Ui, tags: &Vec<String>) {
        ui.horizontal_wrapped(|ui| {
            for tag in tags {
                Self::display_tag(ui, tag);
            }
        });
    }

    fn display_tag(ui: &mut Ui, tag: &str) {
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
            .on_hover_text(RichText::new(Dictionary::get_tag(tag)).size(app::TINY_TEXT_SIZE));

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
