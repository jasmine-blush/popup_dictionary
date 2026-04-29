use egui::Color32;
use egui::Label;
use egui::RichText;
use egui::Ui;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::usize;

use crate::app;
use crate::app::MyApp;
use crate::app::SPACING_SIZE;
use crate::plugin::Plugin;
use crate::plugin::Token;
use crate::plugin::change_progress;
use crate::plugins::kihon_plugin::jmdict_dictionary::AltForm;
use crate::plugins::kihon_plugin::jmdict_dictionary::DictionaryFurigana;
use crate::plugins::kihon_plugin::jmdict_dictionary::{
    Dictionary, DictionaryEntry, DictionaryTerm,
};
use crate::plugins::kihon_plugin::jumandic_tokenizer::tokenize;

const ATTRIBUTIONS_URL: &str =
    "https://github.com/jasmine-blush/popup_dictionary?tab=readme-ov-file#licensing--attributions";

pub struct KihonPlugin {
    tokens: Vec<Token>,
    dictionary: Dictionary,
}

impl Plugin for KihonPlugin {
    fn load_plugin(sentence: &str, progress: Arc<Mutex<String>>) -> Self {
        let result: Result<Self, Box<dyn Error>> = (|| {
            let db_path: PathBuf = match dirs::data_dir() {
                Some(path) => path.join("popup_dictionary").join("db"),
                None => {
                    return Err(Box::from(
                        "No valid data path found in environment variables.",
                    ));
                }
            };

            change_progress(&progress, "Loading dictionary...");
            let dictionary = Dictionary::load_dictionary(&db_path, &progress)?;

            change_progress(&progress, "Tokenizing...");
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

        ui.indent("terms_indent", |ui| {
            /*
            Lookup in database in this order until exists:
            1. surface                                     -- first
            2. base
            3. base minus last letter (e.g. 素敵な)
            4. surface minus last letter                -- last
            */
            if let Some(dictionary_entry) =
                self.dictionary.lookup(&token.input_word).expect(&format!(
                    "Error getting from database when looking up surface: {}",
                    &token.input_word
                ))
            {
                self.display_terms_prioritized(ui, app, &token.input_word, &dictionary_entry);
            } else if let Some(dictionary_entry) = self
                .dictionary
                .lookup(&token.deinflected_word)
                .expect(&format!(
                    "Error getting from database when looking up base: {}",
                    &token.deinflected_word
                ))
            {
                self.display_terms_prioritized(ui, app, &token.deinflected_word, &dictionary_entry);
            } else {
                let mut base_minus_one: String = token.deinflected_word.clone();
                _ = base_minus_one.pop();
                if let Some(dictionary_entry) =
                    self.dictionary.lookup(&base_minus_one).expect(&format!(
                        "Error getting from database when looking up base-1: {}",
                        &base_minus_one
                    ))
                {
                    self.display_terms_prioritized(ui, app, &base_minus_one, &dictionary_entry);
                } else {
                    let mut surface_minus_one: String = token.input_word.clone();
                    _ = surface_minus_one.pop();
                    if let Some(dictionary_entry) =
                        self.dictionary.lookup(&surface_minus_one).expect(&format!(
                            "Error getting from database when looking up surface-1: {}",
                            &surface_minus_one
                        ))
                    {
                        self.display_terms_prioritized(
                            ui,
                            app,
                            &surface_minus_one,
                            &dictionary_entry,
                        );
                    }
                }
            }

            //ui.add_space(app::SPACING_SIZE * 4.0);
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
    fn display_terms_prioritized(
        &self,
        ui: &mut Ui,
        app: &MyApp,
        token: &str,
        entry: &DictionaryEntry,
    ) {
        let mut all_terms: Vec<DictionaryTerm> = entry.terms.clone();
        let init_len: usize = all_terms.len();
        for i in 0..init_len {
            let curr_alt_forms = all_terms[i].alt_forms.clone();
            for alt_form in curr_alt_forms {
                if all_terms
                    .iter()
                    .find(|t| {
                        t.term == alt_form.term
                            && t.reading == alt_form.reading
                            && t.furigana == alt_form.furigana
                    })
                    .is_none()
                {
                    let lookup_string = if alt_form.term.is_empty() {
                        &alt_form.reading
                    } else {
                        &alt_form.term
                    };
                    if let Some(dictionary_entry) =
                        self.dictionary.lookup(lookup_string).expect(&format!(
                            "Error getting from database when looking up alt_form: {:?}, with: {}",
                            alt_form, lookup_string
                        ))
                    {
                        for term in dictionary_entry.terms {
                            if term.term == alt_form.term
                                && term.reading == alt_form.reading
                                && term.furigana == alt_form.furigana
                                && term.id == all_terms[i].id
                            {
                                all_terms.push(term);
                            }
                        }
                    }
                }
            }
        }

        let mut filtered_terms: Vec<DictionaryTerm> = Vec::new();
        for term in &all_terms {
            if let Some((i, existing_term)) = filtered_terms.iter().enumerate().find(|(_i, t)| {
                let alt_form = AltForm {
                    term: term.term.clone(),
                    reading: term.reading.clone(),
                    furigana: term.furigana.clone(),
                };
                t.alt_forms.contains(&alt_form) && t.id == term.id
            }) {
                if let Some(frequency) = term.frequency.get_for_cmp() {
                    if let Some(existing_frequency) = existing_term.frequency.get_for_cmp() {
                        if frequency < existing_frequency {
                            filtered_terms[i] = term.clone();
                        }
                    } else {
                        filtered_terms[i] = term.clone();
                    }
                } else {
                    if existing_term.frequency.get_for_cmp().is_none()
                        && term.common
                        && !existing_term.common
                    {
                        filtered_terms[i] = term.clone();
                    }
                }
            } else {
                filtered_terms.push(term.clone());
            }
        }

        filtered_terms.sort_unstable_by(|a, b| {
            let a_weighted_frequency = match a.frequency.get_for_cmp() {
                Some(frequency) => frequency,
                None => {
                    if a.common {
                        usize::MAX - 1
                    } else {
                        usize::MAX
                    }
                }
            };
            let b_weighted_frequency = match b.frequency.get_for_cmp() {
                Some(frequency) => frequency,
                None => {
                    if b.common {
                        usize::MAX - 1
                    } else {
                        usize::MAX
                    }
                }
            };

            if a.term == token {
                if b.term == token {
                    return a_weighted_frequency.cmp(&b_weighted_frequency);
                } else {
                    return 0.cmp(&1);
                }
            } else {
                if b.term == token {
                    return 1.cmp(&0);
                }
            }

            if a.term.is_empty() && a.reading == token {
                if b.term.is_empty() && b.reading == token {
                    return a_weighted_frequency.cmp(&b_weighted_frequency);
                } else {
                    return 0.cmp(&1);
                }
            } else {
                if b.term.is_empty() && b.reading == token {
                    return 1.cmp(&0);
                }
            }

            if a.reading == token {
                if b.reading == token {
                    return a_weighted_frequency.cmp(&b_weighted_frequency);
                } else {
                    return 0.cmp(&1);
                }
            } else {
                if b.reading == token {
                    return 1.cmp(&0);
                }
            }

            a_weighted_frequency.cmp(&b_weighted_frequency)
        });

        Self::display_terms(ui, app, token, &filtered_terms);
    }

    fn display_terms(ui: &mut Ui, app: &MyApp, token: &str, terms: &Vec<DictionaryTerm>) {
        for (id, dictionary_term) in terms.iter().enumerate() {
            ui.horizontal(|ui| {
                if !dictionary_term.term.is_empty() {
                    if let Some(furigana_vec) = &dictionary_term.furigana {
                        Self::display_furigana(ui, furigana_vec, 1.0, false);
                    } else {
                        let furigana: Vec<DictionaryFurigana> = vec![DictionaryFurigana {
                            ruby: dictionary_term.term.clone(),
                            rt: Some(dictionary_term.reading.clone()),
                        }];
                        Self::display_furigana(ui, &furigana, 1.0, false);
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

                        app.copy_text_safe(term);
                    }
                });
            });

            let mut has_freq = false;
            ui.horizontal(|ui| {
                let (bccjw, jiten) = dictionary_term.frequency.get_all();

                if let Some(frequency) = bccjw {
                    has_freq = true;
                    Self::display_split_tag(
                        ui,
                        "BCCWJ",
                        &format!("{}", frequency),
                        "Word frequency of this form",
                    );
                }
                if let Some(frequency) = jiten {
                    has_freq = true;
                    Self::display_split_tag(
                        ui,
                        "Jiten",
                        &format!("{}", frequency),
                        "Word frequency of this form",
                    );
                }
            });
            if has_freq {
                ui.add_space(app::SPACING_SIZE * 2.0);
            }

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
                                RichText::new(format!("{}", meaning.info.join("; ")))
                                    .size(app::TINY_TEXT_SIZE * 0.9)
                                    .color(app::SECONDARY_TEXT_COLOR),
                            );
                        });
                    });
                }

                count += 1;
            }

            if !dictionary_term.alt_forms.is_empty() {
                ui.add_space(app::SPACING_SIZE * 0.5);

                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(app::SPACING_SIZE);
                        ui.label(
                            RichText::new("Other forms")
                                .size(app::TINY_TEXT_SIZE * 0.8)
                                .color(app::PRIMARY_TEXT_COLOR)
                                .strong(),
                        );
                    });
                });

                let scroll_id = ui.id().with(format!("alt_forms_{}", id));
                let stored_width: f32 = ui
                    .memory(|m| m.data.get_temp(scroll_id))
                    .unwrap_or(ui.available_width());
                egui::ScrollArea::horizontal()
                    .stick_to_right(true)
                    .id_salt(scroll_id)
                    .show(ui, |ui| {
                        let total_width = stored_width.max(ui.available_width());

                        let res = ui.allocate_ui_with_layout(
                            egui::vec2(total_width, 0.0),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.add_space(app::SPACING_SIZE);

                                for (i, form) in dictionary_term.alt_forms.iter().rev().enumerate()
                                {
                                    let is_match = !(!dictionary_term.term.is_empty()
                                        && dictionary_term.term == form.term
                                        || (dictionary_term.term.is_empty()
                                            && dictionary_term.reading == form.reading))
                                        && (form.term == token
                                            || (form.term.is_empty() && form.reading == token));
                                    if let Some(furigana) = &form.furigana {
                                        Self::display_furigana(ui, furigana, 0.8, is_match);
                                    } else {
                                        if form.term.is_empty() {
                                            let mut furigana_vec: Vec<DictionaryFurigana> =
                                                Vec::new();
                                            furigana_vec.push(DictionaryFurigana {
                                                ruby: form.reading.clone(),
                                                rt: None,
                                            });

                                            Self::display_furigana(
                                                ui,
                                                &furigana_vec,
                                                0.8,
                                                is_match,
                                            );
                                        } else {
                                            let mut furigana_vec: Vec<DictionaryFurigana> =
                                                Vec::new();
                                            furigana_vec.push(DictionaryFurigana {
                                                ruby: form.term.clone(),
                                                rt: Some(form.reading.clone()),
                                            });

                                            Self::display_furigana(
                                                ui,
                                                &furigana_vec,
                                                0.8,
                                                is_match,
                                            );
                                        }
                                    }
                                    if i != dictionary_term.alt_forms.len() - 1 {
                                        ui.label(
                                            RichText::new("・")
                                                .size(app::TINY_TEXT_SIZE * 0.8)
                                                .color(app::PRIMARY_TEXT_COLOR),
                                        );
                                    }
                                }
                            },
                        );

                        let measured = res.response.rect.width();
                        ui.memory_mut(|m| m.data.insert_temp(scroll_id, measured));
                    });
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
                let tooltip = Dictionary::get_tag(tag);
                Self::display_tag(ui, tag, tooltip);
            }
        });
    }

    fn display_tag(ui: &mut Ui, tag: &str, tooltip: &str) {
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
            .on_hover_text(RichText::new(tooltip).size(app::TINY_TEXT_SIZE));

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

    fn display_split_tag(ui: &mut Ui, tag: &str, info: &str, tooltip: &str) {
        let left_galley = ui.fonts_mut(|f| {
            f.layout_no_wrap(
                tag.to_string(),
                egui::FontId::proportional(app::TINY_TEXT_SIZE),
                app::PRIMARY_TEXT_COLOR,
            )
        });

        let right_galley = ui.fonts_mut(|f| {
            f.layout_no_wrap(
                info.to_string(),
                egui::FontId::proportional(app::TINY_TEXT_SIZE),
                app::PRIMARY_TEXT_COLOR,
            )
        });

        let padding = egui::Vec2::new(4.0, 0.0);
        let total_width = left_galley.size().x + right_galley.size().x + (4.0 * padding.x);
        let height = left_galley.size().y.max(right_galley.size().y) + (2.0 * padding.y);
        let total_size = egui::Vec2::new(total_width, height);

        let rect = egui::Rect::from_min_size(ui.cursor().min, total_size);
        let response = ui
            .allocate_rect(rect, egui::Sense::hover())
            .on_hover_text(egui::RichText::new(tooltip).size(app::TINY_TEXT_SIZE));

        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Help);
        }

        let left_width = left_galley.size().x + (2.0 * padding.x);
        let left_rect = egui::Rect::from_min_size(rect.min, egui::vec2(left_width, rect.height()));
        let right_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + left_width, rect.min.y),
            egui::vec2(rect.width() - left_width, rect.height()),
        );

        ui.painter().rect_filled(
            left_rect,
            egui::CornerRadius {
                nw: app::CORNER_RADIUS,
                sw: app::CORNER_RADIUS,
                ..Default::default()
            },
            app::SECONDARY_BACKGROUND_COLOR,
        );

        ui.painter().rect_filled(
            right_rect,
            egui::CornerRadius {
                ne: app::CORNER_RADIUS,
                se: app::CORNER_RADIUS,
                ..Default::default()
            },
            Color32::TRANSPARENT,
        );

        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(app::CORNER_RADIUS),
            egui::Stroke::new(2.0, app::SECONDARY_BACKGROUND_COLOR),
            egui::StrokeKind::Middle,
        );

        ui.painter().galley(
            (left_rect.center() - left_galley.size() / 2.0) - egui::Vec2::new(0.0, 1.0),
            left_galley,
            app::PRIMARY_TEXT_COLOR,
        );

        ui.painter().galley(
            (right_rect.center() - right_galley.size() / 2.0) - egui::Vec2::new(0.0, 1.0),
            right_galley,
            app::PRIMARY_TEXT_COLOR,
        );
    }

    fn display_furigana(
        ui: &mut Ui,
        furigana_vec: &Vec<DictionaryFurigana>,
        font_scale: f32,
        is_selected: bool,
    ) {
        let vertical_gap: f32 = 1.0;

        // calculate how wide (and tall) the entire string will be
        let mut total_width: f32 = 0.0;
        let mut max_height: f32 = 0.0;
        let mut galley_data = Vec::new();

        for furigana in furigana_vec {
            let mut main_rich = RichText::new(furigana.ruby.to_string())
                .size(app::BIG_TEXT_SIZE * font_scale)
                .color(app::PRIMARY_TEXT_COLOR);
            if is_selected {
                main_rich = main_rich.underline();
            }
            let main_galley = egui::WidgetText::from(main_rich).into_galley(
                ui,
                None,
                f32::INFINITY,
                egui::FontSelection::Default,
            );

            let furigana_rich = if let Some(reading) = &furigana.rt {
                RichText::new(reading.to_string())
                    .size(app::TINY_TEXT_SIZE * font_scale)
                    .color(app::LIGHT_TEXT_COLOR)
            } else {
                RichText::new("あ")
                    .size(app::TINY_TEXT_SIZE * font_scale)
                    .color(Color32::TRANSPARENT)
            };
            let furigana_galley = egui::WidgetText::from(furigana_rich).into_galley(
                ui,
                None,
                f32::INFINITY,
                egui::FontSelection::Default,
            );

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
                rect.top() + (app::TINY_TEXT_SIZE * font_scale) + vertical_gap,
            );
            ui.painter()
                .galley(main_pos, main_galley, Color32::PLACEHOLDER);

            current_x += char_width;
        }
    }
}
