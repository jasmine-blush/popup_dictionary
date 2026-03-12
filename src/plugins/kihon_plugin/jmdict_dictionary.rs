use serde::{Deserialize, Serialize};
use sled::Db;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;

#[derive(Clone)]
pub struct Dictionary {
    db: Db,
}

#[derive(bincode::Encode, bincode::Decode, Debug)]
pub struct DictionaryEntry {
    pub terms: Vec<DictionaryTerm>,
}

#[derive(bincode::Encode, bincode::Decode, Clone, Debug)]
pub struct DictionaryTerm {
    pub id: String,
    pub frequency: Option<u32>,
    pub common: bool,
    pub term: String,
    pub reading: String,
    pub furigana: Option<Vec<Furigana>>,
    pub meanings: Vec<DictionaryMeaning>,
}

#[derive(bincode::Encode, bincode::Decode, Clone, Debug)]
pub struct DictionaryMeaning {
    pub tags: Vec<String>,
    pub info: Vec<String>,
    pub gloss: Vec<String>,
}

// JMDict json
#[derive(Serialize, Deserialize)]
struct JMDict {
    tags: HashMap<String, String>,
    words: Vec<Word>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Word {
    id: String,
    kanji: Vec<Kanji>,
    kana: Vec<Kana>,
    sense: Vec<Sense>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Kanji {
    common: bool,
    text: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Kana {
    common: bool,
    text: String,
    applies_to_kanji: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Sense {
    part_of_speech: Vec<String>,
    misc: Vec<String>,
    info: Vec<String>,
    applies_to_kanji: Vec<String>,
    applies_to_kana: Vec<String>,
    gloss: Vec<Gloss>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Gloss {
    text: String,
}
// ---

// jmdict-furigana json
#[derive(Serialize, Deserialize, Debug)]
struct JMDictFurigana {
    text: String,
    reading: String,
    furigana: Vec<Furigana>,
}

#[derive(Serialize, Deserialize, bincode::Encode, bincode::Decode, Clone, Debug)]
pub struct Furigana {
    pub ruby: String,
    pub rt: Option<String>,
}
// ---

impl Dictionary {
    pub fn load_dictionary(path: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let db: Db = sled::open(path)?;
        if !db.was_recovered() {
            Self::populate_database(&db)?;
        } else {
            if !db.contains_key("successfully_populated_flag")? {
                db.clear()?;
                Self::populate_database(&db)?;
            }
        }
        Ok(Self { db })
    }

    fn populate_database(db: &Db) -> Result<&Db, Box<dyn Error>> {
        tracing::info!("Trying to populate database for Kihon plugin.");

        Self::parse_jmdict_simplified(&db)?;
        db.insert("successfully_populated_flag", "")?;
        db.flush()?;
        crate::plugins::kihon_plugin::dependencies::cleanup_files();
        Ok(db)
    }

    const GENERIC_TAGS: phf::Map<&'static str, &'static str> = phf::phf_map! {
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

    pub fn get_tag(tag: &str) -> &str {
        match Self::GENERIC_TAGS.get(tag) {
            Some(description) => description,
            None => "unknown",
        }
    }

    fn parse_jmdict_simplified(db: &Db) -> Result<(), Box<dyn Error>> {
        let frequency_map: HashMap<String, u32> = Self::parse_leeds_frequencies()?;
        let furigana_map: HashMap<String, Vec<Furigana>> = Self::parse_jmdict_furigana()?;

        let mut jmdict_simplified_path: PathBuf = match dirs::data_dir() {
            Some(path) => path,
            None => Err("No valid data path found in environment variables.")?,
        };
        jmdict_simplified_path = jmdict_simplified_path
            .join("popup_dictionary")
            .join("dicts")
            .join("jmdict-simplified.json");
        if !jmdict_simplified_path
            .try_exists()
            .is_ok_and(|verified| verified == true)
        {
            crate::plugins::kihon_plugin::dependencies::fetch_jmdict_simplified(
                &jmdict_simplified_path,
            )?;
        }
        let file: File = File::open(jmdict_simplified_path)?;
        let jmdict: JMDict = serde_json::from_reader(BufReader::new(file))?;

        let wildcard: String = String::from("*");
        for word in &jmdict.words {
            let current_id: String = word.id.to_string();
            if !word.kanji.is_empty() {
                for kanji in &word.kanji {
                    for kana in word.kana.iter().filter(|kana| {
                        kana.applies_to_kanji.contains(&wildcard)
                            || kana.applies_to_kanji.contains(&kanji.text)
                    }) {
                        let meanings: Vec<DictionaryMeaning> = Self::build_meanings(
                            &word
                                .sense
                                .iter()
                                .filter(|sense| {
                                    sense.applies_to_kanji.contains(&wildcard)
                                        || sense.applies_to_kanji.contains(&kanji.text)
                                })
                                .collect::<Vec<&Sense>>(),
                            &jmdict.tags,
                        );

                        let mut frequency = frequency_map.get(&kanji.text);
                        if frequency.is_none() {
                            frequency = frequency_map.get(&kana.text);
                        }
                        Self::insert_entry(
                            db,
                            &format!("term:{}", kanji.text),
                            &current_id,
                            &frequency,
                            &kanji.common,
                            &kanji.text,
                            &kana.text,
                            &furigana_map.get(&format!("{},{}", &kanji.text, &kana.text)),
                            &meanings,
                        )?;

                        let mut frequency = frequency_map.get(&kana.text);
                        if frequency.is_none() {
                            frequency = frequency_map.get(&kanji.text);
                        }
                        Self::insert_entry(
                            db,
                            &format!("reading:{}", kana.text),
                            &current_id,
                            &frequency,
                            &kana.common,
                            &kanji.text,
                            &kana.text,
                            &furigana_map.get(&format!("{},{}", &kanji.text, &kana.text)),
                            &meanings,
                        )?;
                    }
                }

                for kana in word
                    .kana
                    .iter()
                    .filter(|kana| kana.applies_to_kanji.is_empty())
                {
                    let meanings: Vec<DictionaryMeaning> = Self::build_meanings(
                        &word
                            .sense
                            .iter()
                            .filter(|sense| {
                                sense.applies_to_kana.contains(&wildcard)
                                    || sense.applies_to_kana.contains(&kana.text)
                            })
                            .collect::<Vec<&Sense>>(),
                        &jmdict.tags,
                    );

                    Self::insert_entry(
                        db,
                        &format!("reading:{}", kana.text),
                        &current_id,
                        &frequency_map.get(&kana.text),
                        &kana.common,
                        "",
                        &kana.text,
                        &None,
                        &meanings,
                    )?;
                }
            } else {
                for kana in &word.kana {
                    let meanings: Vec<DictionaryMeaning> = Self::build_meanings(
                        &word
                            .sense
                            .iter()
                            .filter(|sense| {
                                sense.applies_to_kana.contains(&wildcard)
                                    || sense.applies_to_kana.contains(&kana.text)
                            })
                            .collect::<Vec<&Sense>>(),
                        &jmdict.tags,
                    );

                    Self::insert_entry(
                        db,
                        &format!("reading:{}", kana.text),
                        &current_id,
                        &frequency_map.get(&kana.text),
                        &kana.common,
                        "",
                        &kana.text,
                        &None,
                        &meanings,
                    )?;
                }
            }
        }

        db.flush()?;

        Ok(())
    }

    fn parse_leeds_frequencies() -> Result<HashMap<String, u32>, Box<dyn Error>> {
        let mut frequency_map: HashMap<String, u32> = HashMap::new();
        let mut leeds_frequency_path: PathBuf = match dirs::data_dir() {
            Some(path) => path,
            None => Err("No valid data path found in environment variables.")?,
        };

        leeds_frequency_path = leeds_frequency_path
            .join("popup_dictionary")
            .join("dicts")
            .join("leeds-corpus-frequency.txt");
        if !leeds_frequency_path
            .try_exists()
            .is_ok_and(|verified| verified == true)
        {
            crate::plugins::kihon_plugin::dependencies::fetch_leeds_frequencies(
                &leeds_frequency_path,
            )?;
        }
        let file: File = File::open(leeds_frequency_path)?;

        // note: prone to overflow?
        let mut line_num: u32 = 0;
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            frequency_map.insert(line, line_num);
            line_num += 1;
        }

        Ok(frequency_map)
    }

    fn parse_jmdict_furigana() -> Result<HashMap<String, Vec<Furigana>>, Box<dyn Error>> {
        let mut furigana_map: HashMap<String, Vec<Furigana>> = HashMap::new();

        let mut jmdict_furigana_path: PathBuf = match dirs::data_dir() {
            Some(path) => path,
            None => Err("No valid data path found in environment variables.")?,
        };
        jmdict_furigana_path = jmdict_furigana_path
            .join("popup_dictionary")
            .join("dicts")
            .join("jmdict-furigana.json");
        if !jmdict_furigana_path
            .try_exists()
            .is_ok_and(|verified| verified == true)
        {
            crate::plugins::kihon_plugin::dependencies::fetch_jmdict_furigana(
                &jmdict_furigana_path,
            )?;
        }

        let file: File = File::open(jmdict_furigana_path)?;

        // Handle BOM at file start
        let mut reader = BufReader::new(file);
        let mut bom = [0u8; 3];
        if reader.read_exact(&mut bom).is_ok() && &bom != &[0xEF, 0xBB, 0xBF] {
            reader.seek(SeekFrom::Start(0))?;
        }

        let json: Vec<JMDictFurigana> = serde_json::from_reader(reader)?;

        for jmdict_furigana in json {
            furigana_map.insert(
                format!("{},{}", jmdict_furigana.text, jmdict_furigana.reading),
                jmdict_furigana.furigana,
            );
        }

        Ok(furigana_map)
    }

    fn build_meanings(
        senses: &Vec<&Sense>,
        tags: &HashMap<String, String>,
    ) -> Vec<DictionaryMeaning> {
        let mut meanings: Vec<DictionaryMeaning> = Vec::new();

        for sense in senses {
            let mut meaning_tags: Vec<String> = Vec::new();
            for part in &sense.part_of_speech {
                if let Some(generic_tag) = Self::JMDICT_GENERIC_MAPPING.get(part) {
                    meaning_tags.push(generic_tag.to_string());
                } else {
                    tracing::debug!("No generic tag found for jmdict tag: {}", part);
                }
            }

            let mut info: Vec<String> = sense.info.to_vec();
            info.extend_from_slice(
                &sense
                    .misc
                    .iter()
                    .filter_map(|misc| tags.get(misc))
                    .cloned()
                    .collect::<Vec<String>>(),
            );

            let dict_meaning: DictionaryMeaning = DictionaryMeaning {
                tags: meaning_tags,
                info,
                gloss: sense
                    .gloss
                    .iter()
                    .map(|gloss| gloss.text.to_string())
                    .collect(),
            };

            meanings.push(dict_meaning);
        }

        meanings
    }

    fn insert_entry(
        db: &Db,
        key: &str,
        id: &str,
        frequency: &Option<&u32>,
        common: &bool,
        term: &str,
        reading: &str,
        furigana: &Option<&Vec<Furigana>>,
        meanings: &Vec<DictionaryMeaning>,
    ) -> Result<(), Box<dyn Error>> {
        let frequency: Option<u32> = match frequency {
            Some(freq_value) => Some(**freq_value),
            None => None,
        };
        let furigana: Option<Vec<Furigana>> = match furigana {
            Some(furigana_vec) => Some(furigana_vec.to_vec()),
            None => None,
        };
        let dictionary_term: DictionaryTerm = DictionaryTerm {
            id: id.to_string(),
            frequency,
            common: *common,
            term: term.to_string(),
            reading: reading.to_string(),
            furigana,
            meanings: meanings.to_vec(),
        };

        if let Some(serialized_entry) = db.get(key)? {
            let (mut dictionary_entry, _): (DictionaryEntry, usize) =
                bincode::decode_from_slice(&serialized_entry, bincode::config::standard())?;

            /*
            Sorting of terms in each entry:
            1. common, freq         -- first
            2. common, no freq
            3. uncommon, freq
            4. uncommon, no freq    -- last
            */
            //TODO: implement combining terms with the same meanings into one with "alternative readings"
            if *common {
                if let Some(frequency) = frequency {
                    let mut inserted: bool = false;
                    for (index, term) in dictionary_entry.terms.iter().enumerate() {
                        if !term.common || term.frequency.is_none() {
                            dictionary_entry
                                .terms
                                .insert(index, dictionary_term.clone());
                            inserted = true;
                            break;
                        }
                        if let Some(term_frequency) = term.frequency {
                            if term_frequency > frequency {
                                dictionary_entry
                                    .terms
                                    .insert(index, dictionary_term.clone());
                                inserted = true;
                                break;
                            }
                        }
                    }
                    if !inserted {
                        dictionary_entry.terms.push(dictionary_term.clone());
                    }
                } else {
                    let mut inserted = false;
                    for (index, term) in dictionary_entry.terms.iter().enumerate() {
                        if !term.common {
                            dictionary_entry
                                .terms
                                .insert(index, dictionary_term.clone());
                            inserted = true;
                            break;
                        }
                    }
                    if !inserted {
                        dictionary_entry.terms.push(dictionary_term.clone());
                    }
                }
            } else {
                if let Some(frequency) = frequency {
                    let mut inserted: bool = false;
                    for (index, term) in dictionary_entry.terms.iter().enumerate() {
                        if term.common {
                            continue;
                        }
                        if !term.common && term.frequency.is_none() {
                            dictionary_entry
                                .terms
                                .insert(index, dictionary_term.clone());
                            inserted = true;
                            break;
                        }
                        if let Some(term_frequency) = term.frequency {
                            if term_frequency > frequency {
                                dictionary_entry
                                    .terms
                                    .insert(index, dictionary_term.clone());
                                inserted = true;
                                break;
                            }
                        }
                    }
                    if !inserted {
                        dictionary_entry.terms.push(dictionary_term.clone());
                    }
                } else {
                    dictionary_entry.terms.push(dictionary_term.clone());
                }
            }

            let serialized_entry: Vec<u8> =
                bincode::encode_to_vec(&dictionary_entry, bincode::config::standard())?;
            _ = db.insert(key, serialized_entry.as_slice())?;
        } else {
            let dictionary_entry = DictionaryEntry {
                terms: vec![dictionary_term],
            };
            let serialized_entry: Vec<u8> =
                bincode::encode_to_vec(&dictionary_entry, bincode::config::standard())?;

            _ = db.insert(key, serialized_entry.as_slice())?;
        }

        Ok(())
    }

    pub fn lookup(&self, word: &str) -> Result<Option<DictionaryEntry>, Box<dyn Error>> {
        if let Some(serialized_entry) = self.db.get(format!("term:{}", word))? {
            let (entry, _): (DictionaryEntry, usize) =
                bincode::decode_from_slice(&serialized_entry, bincode::config::standard())
                    .expect(&format!("{:?}", &serialized_entry));
            return Ok(Some(entry));
        }
        if let Some(serialized_entry) = self.db.get(format!("reading:{}", word))? {
            let (entry, _): (DictionaryEntry, usize) =
                bincode::decode_from_slice(&serialized_entry, bincode::config::standard())
                    .expect("reading");
            return Ok(Some(entry));
        }
        Ok(None)
    }

    const JMDICT_GENERIC_MAPPING: phf::Map<&'static str, &'static str> = phf::phf_map! {
        "unc" => "?",
        "n" => "noun",
        "exp" => "expression",
        "adj-na" => "na-adj",
        "adj-no" => "no-adj",
        "adj-i" => "i-adj",
        "v5u" => "godan",
        "vt" => "transitive",
        "pn" => "pronoun",
        "adv" => "adverb",
        "adv-to" => "to-adverb",
        "vs" => "suru",
        "adj-pn" => "pre-noun",
        "int" => "interjection",
        "v1" => "ichidan",
        "vi" => "intransitive",
        "v5s" => "godan",
        "v5k" => "godan",
        "v5r" => "godan",
        "v5aru" => "godan",
        "aux-v" => "aux-verb",
        "adj-f" => "pre-adj",
        "conj" => "conjunction",
        "prt" => "particle",
        "v5m" => "godan",
        "n-suf" => "suffix",
        "v5g" => "godan",
        "v5r-i" => "godan",
        "suf" => "suffix",
        "vs-i" => "suru",
        "adj-t" => "taru-adj",
        "adj-ix" => "i-adj",
        "aux" => "auxiliary",
        "cop" => "copula",
        "pref" => "prefix",
        "vk" => "kuru-verb",
        "aux-adj" => "aux-adj",
        "n-pref" => "prefix",
        "ctr" => "counter",
        "num" => "numeric",
        "vs-s" => "suru",
        "adj-shiku" => "shiku-adj",
        "v5t" => "godan",
        "v5b" => "godan",
        "v5k-s" => "godan",
        "vz" => "ichidan",
        "v2m-s" => "nidan-l",
        "vs-c" => "su-verb",
        "v1-s" => "ichidan",
        "v5n" => "godan",
        "vn" => "irregular",
        "adj-ku" => "ku-adj",
        "v2h-k" => "nidan-u",
        "v2a-s" => "nidan",
        "v4m" => "yodan",
        "v2r-k" => "nidan-u",
        "v4r" => "yodan",
        "v2r-s" => "nidan-l",
        "v5u-s" => "godan",
        "vr" => "irregular",
        "v4s" => "yodan",
        "adj-nari" => "nari-adj",
        "v4k" => "yodan",
        "v2k-s" => "nidan-l",
        "v2t-k" => "nidan-u",
        "v4h" => "yodan",
        "v4t" => "yodan",
        "v4g" => "yodan",
        "v2h-s" => "nidan-l",
        "v2g-s" => "nidan-l",
        "v4b" => "yodan",
        "v2y-s" => "nidan-l",
        "v2d-s" => "nidan-l",
        "v2y-k" => "nidan-u",
        "v2k-k" => "nidan-u",
        "v2g-k" => "nidan-u",
        "v2b-k" => "nidan-u",
        "v2s-s" => "nidan-l",
        "v2z-s" => "nidan-l",
        "v2t-s" => "nidan-l",
        "v2n-s" => "nidan-l",
        "v2w-s" => "nidan-l",
    };
}
