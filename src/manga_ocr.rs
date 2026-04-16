use image::{DynamicImage, GenericImageView, ImageBuffer, imageops::FilterType};
use ndarray::s;
use ort::{inputs, session::Session, value::TensorRef};
#[cfg(feature = "flake-build")]
use std::env;
use std::error::Error;
use std::path::PathBuf;

pub struct MangaOcr {
    encoder_model: Session,
    decoder_model: Session,
    vocab: Vec<String>,
}

impl MangaOcr {
    #[cfg(feature = "flake-build")]
    fn ort_lib_dir() -> PathBuf {
        if let Ok(p) = env::var("ORT_DYLIB_PATH") {
            return PathBuf::from(p);
        }

        // fallback: ./lib next to binary
        let exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        exe.parent()
            .map(|p| p.join("lib"))
            .unwrap_or_else(|| PathBuf::from("./lib"))
    }

    pub fn new() -> Result<Self, Box<dyn Error>> {
        #[cfg(feature = "flake-build")]
        ort::init_from(Self::ort_lib_dir())?.commit();

        // Encoder
        let encoder_model_handle = std::thread::spawn(|| {
            tracing::debug!("Checking for Encoder model.");

            let mut data_path: PathBuf = match dirs::data_dir() {
                Some(path) => path,
                None => Err("No valid data path found in environment variables.").unwrap(),
            };
            data_path = data_path.join("popup_dictionary").join("manga-ocr");

            let encoder_path = data_path.join("encoder_model.onnx");
            if !encoder_path
                .try_exists()
                .is_ok_and(|verified| verified == true)
            {
                tracing::debug!("Attempting Encoder model download.");
                crate::plugins::kihon_plugin::dependencies::fetch_manga_ocr_encoder(&encoder_path)
                    .unwrap();
            }
            Session::builder()
                .unwrap()
                .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
                .unwrap()
                .commit_from_file(encoder_path)
                .unwrap()
        });

        // Decoder
        let decoder_model_handle = std::thread::spawn(|| {
            tracing::debug!("Checking for Decoder model.");

            let mut data_path: PathBuf = match dirs::data_dir() {
                Some(path) => path,
                None => Err("No valid data path found in environment variables.").unwrap(),
            };
            data_path = data_path.join("popup_dictionary").join("manga-ocr");

            let decoder_path = data_path.join("decoder_model.onnx");
            if !decoder_path
                .try_exists()
                .is_ok_and(|verified| verified == true)
            {
                tracing::debug!("Attempting Decoder model download.");
                crate::plugins::kihon_plugin::dependencies::fetch_manga_ocr_decoder(&decoder_path)
                    .unwrap();
            }
            Session::builder()
                .unwrap()
                .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
                .unwrap()
                .commit_from_file(decoder_path)
                .unwrap()
        });

        // Vocab
        let vocab_handle = std::thread::spawn(|| {
            tracing::debug!("Checking for Vocab model.");

            let mut data_path: PathBuf = match dirs::data_dir() {
                Some(path) => path,
                None => Err("No valid data path found in environment variables.").unwrap(),
            };
            data_path = data_path.join("popup_dictionary").join("manga-ocr");

            let vocab_path = data_path.join("vocab.txt");
            if !vocab_path
                .try_exists()
                .is_ok_and(|verified| verified == true)
            {
                tracing::debug!("Attempting Vocab model download.");
                crate::plugins::kihon_plugin::dependencies::fetch_manga_ocr_vocab(&vocab_path)
                    .unwrap();
            }
            std::fs::read_to_string(vocab_path)
                .unwrap()
                .lines()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        });

        let vocab = vocab_handle
            .join()
            .map_err(|e| format!("Could not parse vocab file: {:?}", e))?;
        tracing::debug!("Vocab model session successfully built.");
        let decoder_model = decoder_model_handle
            .join()
            .map_err(|e| format!("Could not build decoder session: {:?}", e))?;
        tracing::debug!("Decoder model session successfully built.");
        let encoder_model = encoder_model_handle
            .join()
            .map_err(|e| format!("Could not build encoder session: {:?}", e))?;
        tracing::debug!("Encoder model session successfully built.");

        Ok(Self {
            encoder_model,
            decoder_model,
            vocab,
        })
    }

    // Code by Mayo Takanashi taken from their crate: https://crates.io/crates/manga-ocr
    // Thank you!
    // *Slightly modified
    pub fn ocr_image(&mut self, img: &DynamicImage) -> Result<String, Box<dyn Error>> {
        tracing::debug!("Running MangaOCR on image.");

        let (width, height) = img.dimensions();
        let max_dim = width.max(height);
        let mut square_canvas = ImageBuffer::new(max_dim, max_dim);

        let rgb_src = img.grayscale().to_rgb8();
        image::imageops::overlay(
            &mut square_canvas,
            &rgb_src,
            ((max_dim - width) / 2) as i64,
            ((max_dim - height) / 2) as i64,
        );

        let image = image::imageops::resize(&square_canvas, 224, 224, FilterType::Lanczos3);

        // original image code
        /*
        let image = img.grayscale().to_rgb8();
        let image =
            image::imageops::resize(&image, 224, 224, image::imageops::FilterType::Lanczos3);
        */

        tracing::debug!("Image pre-processed.");

        // Convert to float32 array and normalize
        let mut tensor = ndarray::Array::zeros((1, 3, 224, 224));
        for (x, y, pixel) in image.enumerate_pixels() {
            let x = x as usize;
            let y = y as usize;

            // Normalize from [0, 255] to [-1, 1]
            tensor[[0, 0, y, x]] = (pixel[0] as f32 / 255.0 - 0.5) / 0.5;
            tensor[[0, 1, y, x]] = (pixel[1] as f32 / 255.0 - 0.5) / 0.5;
            tensor[[0, 2, y, x]] = (pixel[2] as f32 / 255.0 - 0.5) / 0.5;
        }

        // save encoder hidden state
        let inputs = inputs! {
            "pixel_values" => TensorRef::from_array_view(tensor.view())?,
        };
        let outputs = self.encoder_model.run(inputs)?;
        let encoder_hidden_state = outputs[0].try_extract_array::<f32>()?;

        // generate
        let mut token_ids: Vec<i64> = vec![2i64]; // Start token

        tracing::debug!("Encoding done. Entering decoding loop.");

        for _ in 0..300 {
            // Create input tensors
            let input = ndarray::Array::from_shape_vec((1, token_ids.len()), token_ids.clone())?;
            let inputs = inputs! {
                "encoder_hidden_states" => TensorRef::from_array_view(encoder_hidden_state.view())?,
                "input_ids" => TensorRef::from_array_view(input.view())?,
            };

            // Run inference
            let outputs = self.decoder_model.run(inputs)?;

            // Extract logits from output
            let logits = outputs["logits"].try_extract_array::<f32>()?;

            // Get last token logits and find argmax
            let logits_view = logits.view();
            let last_token_logits = logits_view.slice(s![0, -1, ..]);
            let (token_id, _) = last_token_logits
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap_or((0, &0.0));

            token_ids.push(token_id as i64);

            // Break if end token
            if token_id as i64 == 3 {
                break;
            }
        }

        tracing::debug!("Decoding done.");

        // decode tokens
        let text = token_ids
            .iter()
            .filter(|&&id| id >= 5)
            .filter_map(|&id| self.vocab.get(id as usize).cloned())
            .collect::<Vec<_>>();

        let text = text.join("");

        Ok(text)
    }
}
