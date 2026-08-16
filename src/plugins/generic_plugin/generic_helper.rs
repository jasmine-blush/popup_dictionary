// A term defines a unit of language which holds meaning
pub struct GenericTerm {
    // The surface is a representation of the term as found in the dictionary
    // e.g. The surface for 楽しかった is 楽しい
    pub surface: String,

    // The conjugations are applied to the surface to create the term
    // e.g. 楽しい -> 楽しかった
    pub conjugations: Vec<String>,

    // The term can have multiple different definitions assuming various forms
    // e.g. が, 蛾, 賀, 我
    definitions: Vec<GenericDefinition>,
}
impl GenericTerm {
    pub fn new(surface: String, conjugations: Vec<String>) -> GenericTerm {
        GenericTerm {
            surface: surface,
            conjugations: conjugations,
            definitions: Vec::new(),
        }
    }

    pub fn add_definition(
        &mut self,
        word: GenericWord,
        frequencies: Option<Vec<GenericFrequency>>,
        meanings: Vec<GenericMeaning>,
        forms: Option<Vec<GenericWord>>,
    ) {
        self.definitions.push(GenericDefinition {
            word,
            frequencies,
            meanings,
            forms,
        });
    }

    pub fn get_definitions(&self) -> &Vec<GenericDefinition> {
        &self.definitions
    }
}

// A definition is a concrete form of a term which holds one specific set of meanings
// e.g. word: できる -> definitions: [出来る -> to be able to do, 出切る -> to be out of]
pub struct GenericDefinition {
    // Each definition is associated with a certain word (i.e. form of a term)
    // e.g. が, 蛾, 賀, 我
    word: GenericWord,

    // Definitions may have frequencies to help order them against eachother
    frequencies: Option<Vec<GenericFrequency>>,

    // The concrete word this definition defines has one or multiple distinct meanings
    meanings: Vec<GenericMeaning>,

    // This definition's word may have alternative forms
    // 楽しい -> 愉しい, 樂しい
    forms: Option<Vec<GenericWord>>,
}
impl GenericDefinition {
    pub fn get_kanji(&self) -> &Option<(String, Vec<GenericFurigana>)> {
        &self.word.kanji
    }

    pub fn get_kana(&self) -> &str {
        &self.word.kana
    }

    pub fn get_word(&self) -> &GenericWord {
        &self.word
    }

    pub fn get_frequencies(&self) -> &Option<Vec<GenericFrequency>> {
        &self.frequencies
    }

    pub fn get_meanings(&self) -> &Vec<GenericMeaning> {
        &self.meanings
    }

    pub fn get_forms(&self) -> &Option<Vec<GenericWord>> {
        &self.forms
    }
}

// A word is simply a set of characters
pub struct GenericWord {
    // A word may or may not have a kanji representation
    // e.g. 出来る -> (出来る, [{出,で}, {来,き}, {る、る}])
    kanji: Option<(String, Vec<GenericFurigana>)>,

    // Every word has a kana representation
    // e.g. できる
    kana: String,
}
impl GenericWord {
    pub fn new(
        kanji: Option<String>,
        furigana: Option<Vec<GenericFurigana>>,
        kana: String,
    ) -> GenericWord {
        let kanji_opt: Option<(String, Vec<GenericFurigana>)> = kanji.map(|k| {
            (
                k.to_owned(),
                if let Some(f) = furigana {
                    if Self::is_valid_furigana(&k, &f) {
                        f
                    } else {
                        vec![GenericFurigana {
                            base: k.to_owned(),
                            reading: f
                                .iter()
                                .map(|e| e.reading.to_owned())
                                .reduce(|acc, e| format!("{}{}", acc, e))
                                .unwrap_or(kana.to_owned()),
                        }]
                    }
                } else {
                    vec![GenericFurigana {
                        base: k.to_owned(),
                        reading: kana.to_owned(),
                    }]
                },
            )
        });
        GenericWord {
            kanji: kanji_opt,
            kana,
        }
    }

    fn is_valid_furigana(kanji: &str, furigana: &Vec<GenericFurigana>) -> bool {
        if let Some(full_base) = furigana
            .iter()
            .map(|f| f.base.clone())
            .reduce(|acc, e| format!("{}{}", acc, e))
        {
            return kanji == full_base;
        }
        false
    }

    // Returns the full kanji word if there is one, otherwise the kana
    pub fn get_surface(&self) -> &str {
        match &self.kanji {
            Some(kanji) => &kanji.0,
            None => &self.kana,
        }
    }

    pub fn get_kanji(&self) -> &Option<(String, Vec<GenericFurigana>)> {
        &self.kanji
    }

    pub fn get_kana(&self) -> &str {
        &self.kana
    }
}

// A unit of furigana describes the reading of a (set of) character(s)
pub struct GenericFurigana {
    // The unaltered (set of) character(s) the furigana attach to
    pub base: String,

    // The exact reading of the base
    pub reading: String,
}

// A frequency is the ranking of a definition in a certain source
pub struct GenericFrequency {
    // The name of the source in which the definition is ranked by the frequency
    // e.g. BCCWJ
    pub source: String,

    // The ranking of the definition in the source
    // 1 = most frequent, usize::MAX = least frequent
    pub rank: usize,
}

// A meaning represents one specific way to use a word to communicate a certain meaning
pub struct GenericMeaning {
    // Glosses are a concrete set of connected meanings of a word
    // e.g. 出来る -> ["to be able to do", "to be possible", "to be permitted (to do)"]
    glosses: Vec<String>,

    // When the word associated with this meaning is used in this sense, it falls under one or multiple grammatical categories
    // e.g. intransitive, transitive, ichidan, godan, i-adjective, noun, copula, etc.
    categories: Vec<String>,

    // A meaning may hold additional information to help understanding
    // e.g. "word usually written using kana alone"
    infos: Option<Vec<String>>,
}
impl GenericMeaning {
    pub fn new(
        glosses: Vec<String>,
        categories: Vec<String>,
        infos: Option<Vec<String>>,
    ) -> GenericMeaning {
        GenericMeaning {
            glosses: glosses,
            categories: categories.iter().map(|c| GenericCategory::of(c)).collect(),
            infos: infos,
        }
    }

    pub fn get_glosses(&self) -> &Vec<String> {
        &self.glosses
    }

    pub fn get_categories(&self) -> &Vec<String> {
        &self.categories
    }

    pub fn get_infos(&self) -> &Option<Vec<String>> {
        &self.infos
    }
}

// Tries to represent all possible grammatical categories, types, parts of speech, etc.
pub struct GenericCategory {}
impl GenericCategory {
    const CATEGORIES: phf::Map<&'static str, &'static str> = phf::phf_map! {
        "?" => "unclassified",
        "noun" => "noun (common) (futsuumeishi)",
        "expression" => "expression (phrases, clauses, etc.)",
        "na-adj" => "adjectival noun or quasi-adjective (keiyodoshi)",
        "no-adj" =>"noun which may take the genitive case particle 'no'",
        "i-adj" => "adjective (keiyoushi)",
        "godan" => "godan verb",
        "transitive" => "transitive verb",
        "pronoun" => "pronoun",
        "adverb" => "adverb (fukushi)",
        "to-adverb" => "adverb taking the 'to' particle",
        "suru" => "noun or participle which takes the aux. verb suru",
        "pre-noun" => "pre-noun adjectival (rentaishi)",
        "interjection" => "interjection (kandoushi)",
        "ichidan" => "ichidan verb",
        "intransitive" => "intransitive verb",
        "aux-verb" => "auxiliary verb",
        "pre-adj" => "noun or verb acting prenominally",
        "conjunction" => "conjunction",
        "particle" => "particle",
        "suffix" => "suffix",
        "taru-adj" => "'taru' adjective",
        "auxiliary" => "auxiliary",
        "copula" => "copula",
        "prefix" => "prefix",
        "kuru-verb" => "kuru verb - special class",
        "aux-adj" => "auxiliary adjective",
        "counter" => "counter",
        "numeric" => "numeric",
        "shiku-adj" => "'shiku' adjective (archaic)",
        "nidan-l" => "nidan verb (lower class) (archaic)",
        "su-verb" => "su verb - precursor to modern suru",
        "irregular" => "irregular verb",
        "ku-adj" => "'ku' adjective (archaic)",
        "nidan-u" => "nidan verb (upper class) (archaic)",
        "nidan" => "nidan verb (archaic)",
        "yodan" => "yodan verb (archaic)",
        "nari-adj" => "archaic/formal form of na-adjective",
    };

    // Returns a valid category key corresponding to the given string
    pub fn of(key: &str) -> String {
        Self::CATEGORIES.get_key(key).unwrap_or(&"?").to_string()
    }

    pub fn get_info(key: &str) -> String {
        Self::CATEGORIES
            .get(key)
            .unwrap_or(Self::CATEGORIES.get("?").unwrap_or(&""))
            .to_string()
    }
}
