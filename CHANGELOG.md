# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project tries to adhere to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

- **Docs:** A CHANGELOG.md for easier tracking of changes.

### Changed

### Deprecated

### Removed

### Fixed

### Security

---

## [0.1.1] - 2026-03-09

### Added

- **Internal:** Added basic metadata to Cargo.toml.

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

[unreleased]: https://github.com/jasmine-blush/popup_dictionary/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/jasmine-blush/popup_dictionary/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/jasmine-blush/popup_dictionary/releases/tag/v0.1.0
