use egui::{Color32, Label, RichText, Ui};

use crate::{
    app::{self, MyApp},
    plugins::generic_plugin::generic_helper::{
        GenericCategory, GenericDefinition, GenericFurigana, GenericTerm,
    },
};

const COPY_BUTTON_ICON: &str = "\u{1f4cb}";
const COPY_BUTTON_TEXT: &str = "Copy term to clipboard";
const ANKI_BUTTON_ICON: &str = "\u{2295}";
const ANKI_BUTTON_TEXT: &str = "Add definition to Anki";

//用語解説で「増室」に一致する見出し語は見つかりませんでした。以下のキーワードの中にお探しの項目があるかもしれません。

// returns a reference to one of the definitions in the term if that definition should be added to
// Anki
pub fn display_term<'a>(
    app: &MyApp,
    ui: &mut Ui,
    term: &'a GenericTerm,
) -> Option<&'a GenericDefinition> {
    // Display conjugations
    let conjforms_string: String = term.conjugations.join(",");
    if !conjforms_string.is_empty() && conjforms_string != "*" {
        ui.label(
            RichText::new(format!("Conjugations: {}", conjforms_string)).size(app::TINY_TEXT_SIZE),
        );
    } else {
        ui.add_space((app::TINY_TEXT_SIZE) + app::SPACING_SIZE + 1.0);
    }

    // Separator
    ui.scope(|ui| {
        ui.style_mut()
            .visuals
            .widgets
            .noninteractive
            .bg_stroke
            .color = Color32::from_rgba_premultiplied(10, 10, 10, 10);
        ui.separator();
    });

    let mut definition_to_add_to_anki: Option<&GenericDefinition> = None;
    ui.indent("definitions_indent", |ui| {
        definition_to_add_to_anki = display_definitions(app, ui, term);
    });

    definition_to_add_to_anki
}

fn display_definitions<'a>(
    app: &MyApp,
    ui: &mut Ui,
    term: &'a GenericTerm,
) -> Option<&'a GenericDefinition> {
    let mut definition_to_add_to_anki: Option<&GenericDefinition> = None;
    for (id, definition) in term.get_definitions().iter().enumerate() {
        // Display header row
        ui.horizontal(|ui| {
            // Display word as kanji with furigana or just kana
            if let Some(kanji) = definition.get_kanji() {
                display_kanji_with_furigana(ui, &kanji.0, &kanji.1, 1.0, false);
            } else {
                ui.label(RichText::new(definition.get_kana()).heading());
            }

            // Show buttons on the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(app::SPACING_SIZE);

                if ui
                    .add(egui::Button::new(
                        RichText::new(COPY_BUTTON_ICON).size(app::TINY_TEXT_SIZE),
                    ))
                    .on_hover_text(RichText::new(COPY_BUTTON_TEXT).size(app::TINY_TEXT_SIZE))
                    .clicked()
                {
                    app.copy_text_safe(definition.get_word().get_surface());
                }

                if ui
                    .add(egui::Button::new(
                        RichText::new(ANKI_BUTTON_ICON).size(app::TINY_TEXT_SIZE),
                    ))
                    .on_hover_text(RichText::new(ANKI_BUTTON_TEXT).size(app::TINY_TEXT_SIZE))
                    .clicked()
                {
                    definition_to_add_to_anki = Some(definition);
                }
            });
        });

        // Display frequency tags
        if let Some(frequencies) = definition.get_frequencies() {
            if frequencies.len() > 0 {
                ui.horizontal(|ui| {
                    for frequency in frequencies {
                        display_split_tag(
                            ui,
                            &frequency.source,
                            &frequency.rank.to_string(),
                            &format!("Frequency of this word according to {}", frequency.source),
                        );
                    }
                });

                ui.add_space(app::SPACING_SIZE * 2.0);
            }
        }

        // Display meanings
        let mut counter: usize = 0;
        let mut prev_categories: String = String::new();
        for meaning in definition.get_meanings() {
            // Display tags only if they aren't the same as previous meanings
            let categories: String = meaning.get_categories().join("");
            if categories != prev_categories {
                prev_categories = categories.to_owned();
                if counter > 0 {
                    ui.add_space(app::SPACING_SIZE);
                    counter = 1;
                }

                ui.horizontal_wrapped(|ui| {
                    for category in meaning.get_categories() {
                        display_tag(ui, category, &GenericCategory::get_info(&category));
                    }
                });
            }
            if counter == 0 {
                counter = 1;
            }

            // Display glosses
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!("{}.", counter))
                        .small()
                        .color(app::SECONDARY_TEXT_COLOR),
                );
                ui.label(RichText::new(meaning.get_glosses().join(", ")).small());
            });

            // Display extra info
            if let Some(infos) = meaning.get_infos() {
                if infos.len() > 0 {
                    ui.horizontal_top(|ui| {
                        ui.add(
                            Label::new(
                                RichText::new(format!("{}.", counter))
                                    .small()
                                    .color(Color32::TRANSPARENT),
                            )
                            .selectable(false),
                        );
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(infos.join("; "))
                                    .size(app::TINY_TEXT_SIZE * 0.9)
                                    .color(app::SECONDARY_TEXT_COLOR),
                            );
                        });
                    });
                }
            }

            counter += 1;
        }

        // Display alt forms of the word if there are any
        if let Some(forms) = definition.get_forms() {
            if forms.len() > 0 {
                ui.add_space(app::SPACING_SIZE * 0.5);

                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left((egui::Align::Center)), |ui| {
                        ui.add_space(app::SPACING_SIZE);
                        ui.label(
                            RichText::new("Other forms")
                                .size(app::TINY_TEXT_SIZE * 0.8)
                                .color(app::PRIMARY_TEXT_COLOR)
                                .strong(),
                        );
                    });
                });

                let scroll_id_salt = ui.id().with(format!("alt_forms_{}", id));
                let stored_width: f32 = ui
                    .memory(|m| m.data.get_temp(scroll_id_salt))
                    .unwrap_or_else(|| ui.available_width());
                egui::ScrollArea::horizontal()
                    .stick_to_right(true)
                    .id_salt(scroll_id_salt)
                    .show(ui, |ui| {
                        let total_width = stored_width.max(ui.available_width());

                        let res = ui.allocate_ui_with_layout(
                            egui::vec2(total_width, 0.0),
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.add_space(app::SPACING_SIZE);

                                for (i, form) in forms.iter().rev().enumerate() {
                                    // if the alt form matches the term's surface and the alt form
                                    // doesn't already match the definition's word, then underline it
                                    let form_matches_term = !(definition.get_word().get_surface()
                                        == form.get_surface())
                                        && (form.get_surface() == term.surface);

                                    if let Some(furigana) = form.get_kanji() {
                                        display_kanji_with_furigana(
                                            ui,
                                            &furigana.0,
                                            &furigana.1,
                                            0.8,
                                            form_matches_term,
                                        );
                                    } else {
                                        // if it's a kana-only word, use furigana function anyways
                                        // but with empty furigana in order to assure consistent formatting
                                        display_kanji_with_furigana(
                                            ui,
                                            form.get_kana(),
                                            &vec![GenericFurigana {
                                                base: form.get_kana().to_owned(),
                                                reading: "".to_owned(),
                                            }],
                                            0.8,
                                            form_matches_term,
                                        );
                                    }

                                    if i != forms.len() - 1 {
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
                        ui.memory_mut(|m| m.data.insert_temp(scroll_id_salt, measured));
                    });
            }
        }

        ui.add_space(app::SPACING_SIZE * 0.5);

        let percent: f32 = 0.8;
        let width: f32 = ui.available_width() * percent;
        let margin: f32 = (ui.available_width() - width) / 2.0;

        ui.horizontal(|ui| {
            ui.add_space(margin);
            let rect: egui::Rect = ui.allocate_space(egui::vec2(width, 1.)).1;
            ui.painter().line_segment(
                [rect.left_center(), rect.right_center()],
                egui::Stroke::new(1.0, Color32::from_rgba_premultiplied(20, 20, 20, 20)),
            );
        });

        ui.add_space(app::SPACING_SIZE * 0.5);
    }
    definition_to_add_to_anki
}

fn display_kanji_with_furigana(
    ui: &mut Ui,
    kanji: &str,
    furigana: &Vec<GenericFurigana>,
    font_scale: f32,
    selected: bool,
) {
    let vertical_gap: f32 = 1.0;

    // calculate how wide (and tall) the entire string will be
    let mut total_width: f32 = 0.0;
    let mut max_height: f32 = 0.0;
    let mut galley_data = Vec::new();

    for furigana in furigana {
        let mut main_rich = RichText::new(furigana.base.to_owned())
            .size(app::BIG_TEXT_SIZE * font_scale)
            .color(app::PRIMARY_TEXT_COLOR);
        if selected {
            main_rich = main_rich.underline();
        }
        let main_galley = egui::WidgetText::from(main_rich).into_galley(
            ui,
            None,
            f32::INFINITY,
            egui::FontSelection::Default,
        );

        let furigana_color = if furigana.reading == furigana.base {
            Color32::TRANSPARENT
        } else {
            app::LIGHT_TEXT_COLOR
        };
        let furigana_rich = RichText::new(furigana.reading.to_owned())
            .size(app::TINY_TEXT_SIZE * font_scale)
            .color(furigana_color);
        let furigana_galley = egui::WidgetText::from(furigana_rich).into_galley(
            ui,
            None,
            f32::INFINITY,
            egui::FontSelection::Default,
        );

        let part_width: f32 = main_galley.size().x.max(furigana_galley.size().x);
        let part_height: f32 = main_galley.size().y + furigana_galley.size().y + vertical_gap;

        total_width += part_width;
        max_height = max_height.max(part_height);

        galley_data.push((main_galley, furigana_galley, part_width));
    }

    // then draw without gap between galleys
    let (rect, _) = ui.allocate_exact_size(
        egui::Vec2::new(total_width, max_height),
        egui::Sense::empty(),
    );

    let mut current_x: f32 = rect.left();
    for (main_galley, furigana_galley, part_width) in galley_data {
        let furigana_pos = egui::Pos2::new(
            current_x + (part_width - furigana_galley.size().x) * 0.5,
            rect.top(),
        );
        ui.painter()
            .galley(furigana_pos, furigana_galley, Color32::PLACEHOLDER);

        let main_pos = egui::Pos2::new(
            current_x + (part_width - main_galley.size().x) * 0.5,
            rect.top() + (app::TINY_TEXT_SIZE * font_scale) + vertical_gap,
        );
        ui.painter()
            .galley(main_pos, main_galley, Color32::PLACEHOLDER);

        current_x += part_width;
    }
}

fn display_split_tag(ui: &mut Ui, left_text: &str, right_text: &str, tooltip: &str) {
    let left_galley = ui.fonts_mut(|f| {
        f.layout_no_wrap(
            left_text.to_owned(),
            egui::FontId::proportional(app::TINY_TEXT_SIZE),
            app::PRIMARY_TEXT_COLOR,
        )
    });

    let right_galley = ui.fonts_mut(|f| {
        f.layout_no_wrap(
            right_text.to_owned(),
            egui::FontId::proportional(app::TINY_TEXT_SIZE),
            app::PRIMARY_TEXT_COLOR,
        )
    });

    let padding = egui::Vec2::new(4.0, 0.0);
    let total_width = left_galley.size().x + right_galley.size().x + (4.0 * padding.x);
    let height = left_galley.size().y.max(right_galley.size().y) + (2.0 * padding.y);
    let total_size = egui::Vec2::new(total_width, height);

    let rect = egui::Rect::from_min_size(ui.cursor().min, total_size);
    let res = ui
        .allocate_rect(rect, egui::Sense::hover())
        .on_hover_text(egui::RichText::new(tooltip).size(app::TINY_TEXT_SIZE));

    if res.hovered() {
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

fn display_tag(ui: &mut Ui, text: &str, tooltip: &str) {
    let text_galley = ui.fonts_mut(|f| {
        f.layout_no_wrap(
            text.to_owned(),
            egui::FontId::proportional(app::TINY_TEXT_SIZE),
            app::PRIMARY_TEXT_COLOR,
        )
    });

    let padding = egui::Vec2::new(4.0, 0.0);
    let rect = egui::Rect::from_min_size(ui.cursor().min, text_galley.size() + (2.0 * padding));
    let res = ui
        .allocate_rect(rect, egui::Sense::hover())
        .on_hover_text(RichText::new(tooltip).size(app::TINY_TEXT_SIZE));

    if res.hovered() {
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
}
