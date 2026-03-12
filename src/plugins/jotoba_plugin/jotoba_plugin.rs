use egui::Color32;
use egui::Context;
use egui::RichText;
use egui::Ui;
use egui::containers::Frame;
use std::cell::RefCell;

use crate::app::MyApp;
use crate::plugin::Plugin;
use crate::plugin::Token;
use crate::plugins::jotoba_plugin::jotoba_tokenizer::JotobaTokenizer;

pub struct JotobaPlugin {
    tokens: Vec<Token>,
    jotoba_tokenizer: RefCell<JotobaTokenizer>, // TODO: REMOVE THIS REFCELL WHEN POSSIBLE
}

impl Plugin for JotobaPlugin {
    fn load_plugin(sentence: &str) -> Self {
        let mut jotoba_tokenizer: JotobaTokenizer = JotobaTokenizer::new();
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

    fn display_token(&self, ctx: &Context, frame: &Frame, app: &MyApp, ui: &mut Ui, token: &Token) {
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
                        if let Some(kanji) = &word.reading.kanji {
                            ui.label(RichText::new(kanji).heading()); //.size(22.0));
                        } else {
                            ui.label(
                                RichText::new(&word.reading.kana).heading(), //.size(22.0)
                            );
                        }
                        let mut count: u32 = 1;
                        for sense in &word.senses {
                            ui.label(
                                RichText::new(format!("{}. {}", count, sense.glosses.join(", ")))
                                    .small(),
                            );
                            count += 1;
                        }
                    }
                    /* });*/
                }
                Err(e) => tracing::debug!("Could not display token due to error: {e}"),
            };
        }
    }

    fn open(&self, ctx: &Context) {
        tracing::info!("Trying to open Jotoba website with input text.");

        ctx.open_url(egui::output::OpenUrl {
            url: format!(
                "https://jotoba.de/search/0/{}?l=en-US",
                self.get_sentence_string()
            ),
            new_tab: true,
        });
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
}
