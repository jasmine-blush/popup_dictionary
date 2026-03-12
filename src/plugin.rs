use egui::containers::Frame;
use egui::{Context, Ui};
use std::time::{Duration, Instant};

use crate::app::MyApp;

pub trait Plugin: Send + 'static {
    fn load_plugin(sentence: &str) -> Self
    where
        Self: Sized;
    fn get_tokens(&self) -> &Vec<Token>;
    fn display_token(&self, ctx: &Context, frame: &Frame, app: &MyApp, ui: &mut Ui, token: &Token);
    fn open(&self, ctx: &Context);
}

#[derive(Clone, Copy, PartialEq)]
pub enum Plugins {
    Kihon,
    Jotoba,
}

impl Plugins {
    pub fn all() -> Vec<Self> {
        vec![Plugins::Kihon, Plugins::Jotoba]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Plugins::Kihon => "kihon",
            Plugins::Jotoba => "jotoba",
        }
    }

    pub fn generate(&self, sentence: &str) -> Box<dyn Plugin> {
        let start: Instant = Instant::now();

        let result: Box<dyn Plugin> = match self {
            Plugins::Kihon => Box::new(
                crate::plugins::kihon_plugin::kihon_plugin::KihonPlugin::load_plugin(sentence),
            ),
            Plugins::Jotoba => Box::new(
                crate::plugins::jotoba_plugin::jotoba_plugin::JotobaPlugin::load_plugin(sentence),
            ),
        };

        let duration: Duration = start.elapsed();
        tracing::debug!(
            "Plugin loaded in: {:.3} ms for sentence length {}",
            duration.as_secs_f64() * 1000.0,
            sentence.len()
        );

        result
    }
}

#[derive(Clone, Debug)]
pub enum Validity {
    VALID,
    INVALID,
    UNKNOWN,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub input_word: String,        // term as input by user (surface)
    pub deinflected_word: String,  // deinflected surface as given by tokenizer (base)
    pub conjugations: Vec<String>, // conjforms
    pub validity: Validity,
}

impl Token {
    pub fn is_valid(&self) -> bool {
        // UNKNOWN words might be valid, so they shouldn't count as invalid
        match self.validity {
            Validity::INVALID => false,
            _ => true,
        }
    }
}
