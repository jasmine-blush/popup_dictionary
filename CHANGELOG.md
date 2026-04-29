# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project tries to adhere to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

- **Jotoba:** Furigana, tags and extra information to definitions.
- **Core:** A Nix flake including a dev environment by @IamTheCarl in https://github.com/jasmine-blush/popup_dictionary/pull/1.
- **Core:** File integrity verification using SHA256 after dependency download
  (currently Kihon and MangaOCR).
- **Kihon:** A version flag into the Kihon database. It will now automatically
  regenerate when a newer database version is required.
- **CLI:** A `--keep-open` argument which updates the app with new inputs from
  the clipboard in watch mode even while the window is already/still open.
- **UI:** Plugins can now display helpful status information while loading.
- **UI:** A pause button inside the UI when the new `--keep-open` option is set.
- **Kihon:** BCCWJ as the primary frequency dataset (replacing the leeds corpus).
- **Kihon:** Frequencies (currently BCCWJ) are now shown for each definition term.

### Changed

- **Jotoba:** Started aligning the way definitions are presented in the Jotoba
  plugin with the Kihon plugin to lay the groundwork for future unification of
  definition display.
- **Kihon:** Different forms (kanji/reading) of a word that hold the same
  meaning are not displayed separately anymore. Instead, the primary term
  will now show its "Other forms" in the definition.
- **Internal:** Pinned MangaOCR dependencies to a specific version to avoid
  potential unwanted updates.
- **Core:** The default launch behaviour when no arguments are provided now
  includes `--keep-open` and `--at-mouse`.
- **Core:** Lowered clipboard polling delay in watch mode to 100ms from 200ms (experimental).
- **Kihon:** Significantly improved performance of the Kihon database generation
  via multi-threading and general code improvements. Dataset files are now
  not saved to disk anymore.
- **UI:** Moved the copy input button to the input bar at the top of the UI.
- **UI:** The open in web button now uses a globe symbol (from an i symbol).
- **Kihon:** Definition terms are now sorted much more naturally using
  their frequencies, terms that match the input word are displayed at the top.
- **Kihon:** Definition terms now display their most frequently used form
  (kanji/kana) as the main term. In cases where the main term differs from the
  input word but one of the "Other forms" matches it, that form is underlined.

### Deprecated

### Removed

- **Kihon:** The University of Leeds Corpus frequency dataset.

### Fixed

- **Kihon:** Some alignment text being selectable that is meant to be invisible.

### Security

---

## [0.2.1] - 2026-04-13

### Fixed

- **Jotoba:** Input text containing percent signs not opening properly in the browser
  due to missing URL encoding.
- **Core:** File logging not happening at the trace level.
- **Core:** The crash when editing the input text to be empty.
- **Jotoba:** A bug where English words in the input text would disappear in some
  cases.
- **Jotoba:** An API request spam while having an invalid word selected.

---

## [0.2.0] - 2026-04-09

### Added

- **Docs:** A `CHANGELOG.md` for easier tracking of changes.
- **CLI:** A `--log-file` argument allowing for verbose logging to a file.
- **Kihon:** A post-generation cleanup step that deletes three dataset files
  needed only for initial database population.
- **Core:** More and improved logging throughout the codebase.
- **Core:** A custom font loading mechanism that checks for supported fonts on
  the system, if none are found a default font is downloaded.
- **CLI:** A `--font` argument for specifying a system font to be used.
- **UI:** A pause/resume button to the tray menu to pause detection in watch mode.
- **UI:** Helpful tooltips when hovering over buttons.
- **Kihon:** A copy button to each definition term.
- **Core:** `MangaOCR` as a new OCR engine.
- **UI:** A tray menu button which allows switching between Tesseract and MangaOCR.
- **UI:** An edit button to manually edit the input text.
- **UI:** A reverse button to reverse the input text. Useful in some cases where
  Tesseract recognizes horizontal text correctly but outputs it in reverse due to
  wrongful parsing as vertical text.
- **CLI:** An `--ocr-engine` argument for specifying the OCR engine to be used.

### Changed

- **Core:** Migrated logging system from `log`/`env_logger` to `tracing`.
- **Core:** Implemented pre-process upscaling of input image for Tesseract which
  vastly improves recognition of smaller font sizes.

### Removed

- **Core:** The statically linked default font. This effectively reduces the
  size of the binary/executable by ~30MiB.

### Fixed

- **Core:** A duplicate check for whether Tesseract is installed.
- **Core:** A rare case where Tesseract would not parse horizontal text when
  vertical confidence is NaN.
- **Core:** The bug where scrolling horizontally and then clicking on a token in
  the input text would make the scroll-bar jump.

---

## [0.1.1] - 2026-03-09

### Added

- **Internal:** Basic metadata to `Cargo.toml`.

### Changed

- **Linux:** Watch mode with tray icon is now the default when no arguments are
  provided.
- **Windows:** Improved Tesseract detection by checking the default install path
  automatically.
- **Windows:** Suppressed the brief console window flicker when OCR is used.
- **Docs:** General improvements and updates to the README.

### Fixed

- **Linux:** Fixed a bug where the window position was being continuously set on
  x11.
- **Windows:** Fixed a regression in the tray icon functionality.

---

## [0.1.0] - 2026-02-28

- **Core:** Initial pre-release of the project.

---

[unreleased]: https://github.com/jasmine-blush/popup_dictionary/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/jasmine-blush/popup_dictionary/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/jasmine-blush/popup_dictionary/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/jasmine-blush/popup_dictionary/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/jasmine-blush/popup_dictionary/releases/tag/v0.1.0
