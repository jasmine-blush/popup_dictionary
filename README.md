<h2 align="center">Popup Dictionary</h2>

This application is a WIP **pop-up dictionary** (currently for Japanese->English) that works outside your browser.  

It is inspired by tools like [Yomitan](https://github.com/yomidevs/yomitan), an incredibly useful browser extension that lets you look-up the definition of words with the press of a button.
Since Yomitan only works in the browser, I decided to make a pop-up dictionary application that can be used outside the browser, utilizing methods like **OCR (Optical Character Recognition) or the system clipboard** for looking up text.  
The main difference to something like Yomitan is that the look-up is not restricted to a single word, but you can copy/OCR a whole sentence or even full blocks of text and look-up the individual words.

## Usage
> [!NOTE]\
> Currently, the main focus of development is on **Linux** (X11 and Wayland). Basic **Windows** support is already there, however some features may or may not fully work until I get to it.

These two videos (slightly outdated) showcase the popup dictionary being used to look up sentences and words by copying or screenshotting text in various applications:

https://github.com/user-attachments/assets/df14e686-d6c0-497a-87ff-5e320c2e02e2

https://github.com/user-attachments/assets/33a60c3a-f775-4ef4-99d8-dd7cbb0fe1f2

### Easy Usage Example
There's two main ways you can easily use this as a popup dictionary similar to something like Yomitan:
1. When you're about to e.g. read a book in Japanese, simply launch the binary/executable to open the application in watcher mode. In this mode, the application stays running in the background and waits for you to copy or screenshot any Japanese text. Everytime you do, the popup dictionary opens with that text as input. You can close the window or keep it open and copy/screenshot new text as often as you like. Once you're done reading, simply exit the application via the tray icon. It is also possible to pause/unpause the watcher via the buttons in the app or the tray menu.
2. [WIP on Windows] Assign **keybindings** to the commands ``popup_dictionary --clipboard`` and/or ``hyprshot -m region -r -- | popup_dictionary --ocr`` (replacing ``hyprshot`` with your preferred screenshot tool). This way you can copy any Japanese text, then press your keybind to open it in the popup dictionary; or press the second keybind to screenshot any Japanese text and open it.

### Plugins
There are currently two "Plugins" you can use for looking up text, these correspond to the two tabs at the bottom of the popup window:
- The **Kihon** plugin is the default when launching the application. It runs entirely locally on your machine (after the initial one-time dataset download) and uses a mix of hand-picked methods and dictionaries for tokenization and looking up words.
- The **Jotoba** plugin uses the API of the website [jotoba.de](https://jotoba.de/) for both tokenization and looking up words. To use it, you need an active internet connection.

More plugins will be added in the future, as well as the existing plugins improved and expanded on.

By using the ``--initial-plugin`` argument, you can specify which plugin the application should start with.

### OCR
When screenshotting or copying an image while using watch mode (e.g. if you launched the binary/executable by double-clicking) or when using ocr mode, an OCR engine is used to parse Japanese text from the input image. Two different OCR engines are currently implemented: ``Tesseract`` and ``MangaOCR``. You can switch between these two OCR engines inside the tray menu or by specifying which one to use via the ``--ocr-engine`` command-line option.
Which OCR engine is better for your use-cases depends on what kind of text you're trying to look up.

#### Tesseract
This is the default OCR engine the application tries to use. See the [Installation](#installation) section on how to install it.
- **Pros:** Tesseract is very fast and uses minimal resources (CPU/RAM). It works extremely well on dark text on a light background or vice versa. It can parse even huge blocks of text.
- **Cons:** Tesseract can struggle with colored text, colored backgrounds, some stylized fonts and fonts with outlines around the characters.

#### MangaOCR
> [!NOTE]\
> When first using MangaOCR, three model files totalling around ~440MB are downloaded under ``~/.local/share/popup_dictionary/`` (Linux) or ``%APPDATA%\popup_dictionary\`` (Windows). This may take a few minutes depending on your internet connection and device specifications. There is currently no way to see the download progress, please wait a few minutes for the download to finish before the application window opens.
  
The MangaOCR engine requires no manual installation.
- **Pros:** MangaOCR is great at recognizing very short pieces of text (i.e. one sentence or less). It can handle stylized fonts, colors, etc. It can parse extremely tiny font sizes a little better than Tesseract. It is also better at parsing Japanese text with furigana.
- **Cons:** MangaOCR takes up around ~400MB of RAM and is slower than Tesseract. It has a maximum image size of 224x224, so images/screenshots bigger than this get squished which reduces accuracy.

### Modes (Advanced Users)
The program must be launched in exactly one of **six different modes**. When no mode is specified, the program defaults to ``watch`` mode with ``--tray`` and ``--keep-open``. These modes determine how the popup dictionary receives the input text you would like to look up.
You can choose a mode using one of the following arguments:
- ``--text`` or ``-t``: Put some text after this argument (don't forget quotation marks if your text includes spaces) to pass it directly to the program.
  - Example: ``popup_dictionary --text "予約より一日早く発ちます。"``
- ``--primary`` or ``-p``: In this mode, any text that is currently in the **primary selection** is taken and passed to the program. This is **Linux-only** and may or may not work on Wayland depending on your compositor. The primary selection usually contains any text you have **currently highlighted** (e.g. with your mouse).
- ``--secondary`` or ``-s``: In this mode, any text that is currently in the **secondary selection** is taken and passed to the program. This is **X11-only** and is rarely implemented/used.
- ``--clipboard`` or ``-b``: In this mode, any text that is currently in your **main clipboard** is taken and passed to the program. This uses what you would usually call the "clipboard" on any OS.
- ``--ocr`` or ``-o``: In this mode, an OCR engine (``tesseract`` by default) is used to parse text from an input image. You can either specify the **path to an image file** after this argument, or you can pipe in **raw image data** from ``stdin``.
  - Example: ``popup_dictionary --ocr ~/Pictures/japanese_text.png`` or ``hyprshot -m region -r -- | popup_dictionary --ocr``
- ``--watch`` or ``-w``: In this mode, the program stays running in the background and waits for any **valid text** or **raw image data** to enter the **main clipboard**. When either of those is detected, the popup dictionary window opens using either the text as input or running OCR mode on the image. The program stays running in the background even when the window is closed and waits for new valid clipboard content. Specifying the option ``--tray`` can be useful in this mode, as this allows you to easily end the background process via the tray icon. If the option ``--keep-open`` is specified, the watcher will update the input text even while the window is still open.

## Installation
### Linux
Head over to the **Releases** tab and pick out the binary/archive matching your system.
For Linux, there are three different versions:
- **Generic Linux**: This one can be used on any Linux environment (X11 or Wayland).
- **Wayland**: Same as the regular Linux one but with better Wayland support.
- **Hyprland**: Same again but with better Hyprland support.

Once downloaded, simply extract the archive and execute the contained binary with appropriate arguments. Using ``--help`` will output the usage instructions and a summary of available command-line options.
#### OCR
In order to use **OCR mode** with the default OCR engine ``tesseract``, it needs to be installed on your system and be in your **PATH**. A package for tesseract should be available on most Linux distributions (e.g., via ``apt install tesseract-ocr`` on **Debian/Ubuntu**, ``dnf install tesseract`` on **Fedora**, or ``pacman -S tesseract`` on **Arch Linux**).

Additionally, the **English**, **Japanese** (for horizontal text) and **Japanese Vertical** (for vertical text) language packs need to be installed. This can usually be done via commands like this:
- **Debian/Ubuntu**: ``sudo apt install tesseract-ocr-eng tesseract-ocr-jpn tesseract-ocr-jpn-vert``
- **Fedora**: ``sudo dnf install tesseract-langpack-eng tesseract-langpack-jpn tesseract-langpack-jpn_vert``
- **Arch Linux**: ``sudo pacman -S tesseract-data-eng tesseract-data-jpn tesseract-data-jpn_vert``

Afterwards, you can verify that everything was installed correctly using:
```sh
tesseract --list-langs
```
<br />

Using **MangaOCR** as the OCR engine requires no manual installation. When first using it, the required model files are downloaded automatically in the background. Please wait a few minutes for the download to finish before the application window opens.

### Windows
Head over to the **Releases** tab, download the windows archive and extract it. If you're fine with using the ``watch`` mode (see the [Easy Usage](#easy-usage-example) and [Modes](#modes-advanced-users) sections above), simply double-click on the executable to launch the application. You can also create a **Shortcut** of the executable and pin it to your Start Menu or Taskbar for easier access.  
Running the executable in CMD or Powershell with the ``--help`` argument will output the usage instructions and a summary of available command-line options.

#### OCR
In order to use **OCR mode** with the default OCR engine ``tesseract``, it needs to be installed on your system in the default install directory or be in your **PATH**.  
To install tesseract on Windows, download the installer here: [tesseract-ocr-w64-setup-5.5.0.20241111.exe](https://github.com/tesseract-ocr/tesseract/releases/download/5.5.0/tesseract-ocr-w64-setup-5.5.0.20241111.exe).  
In the installer, select at least these components to be able to use OCR mode:  

<img width="499" height="388" alt="windows_tesseract1" src="https://github.com/user-attachments/assets/a611741c-4f9c-4e47-b657-3e0adfd9296b" />
<img width="499" height="388" alt="windows_tesseract2" src="https://github.com/user-attachments/assets/1c4bcd9b-8008-49d4-8050-318039ff4c4e" />
<br />
<br />
  
Using **MangaOCR** as the OCR engine requires no manual installation. When first using it, the required model files are downloaded automatically in the background. Please wait a few minutes for the download to finish before the application window opens.

## Troubleshooting
[WIP]

## Building
This project is developed in 🔥blazingly-fast, memory-safe🔥 Rust. Building and running it from source should be relatively simple using the Rust toolchain/Cargo. You can find installation instructions here [rustup.rs](https://rustup.rs/).

#### Dependencies:
[WIP]
- Openssl development libraries such as ``libssl-devel`` (Ubuntu) or ``openssl-devel`` (Fedora)
- ``fontconfig-devel``
- ``libxkbcommon-devel``
- ``libstdc++-devel``
- ``gcc-c++``

#### Steps:
1. Clone the repository:
   ```sh
   git clone https://github.com/jasmine-blush/popup_dictionary.git
   cd popup_dictionary
   ```
2. Build it:
   ```sh
   cargo build
   ```
   The compiled binary/executable will be inside the ``target`` directory.
3. Or run it directly:
   ```sh
   cargo run
   ```
   In order to pass arguments to the popup dictionary when running it like this, use:
   ```sh
   cargo run -- [arguments]
   ```
You can use ``--release`` for an optimized release build.
There are two optional feature-flags ``wayland-support`` and ``hyprland-support`` that enable better wayland and hyprland support respectively. You can use them like this:
   ```sh
   cargo build --features hyprland-support
   ```

All in all, a command could look something like this:
   ```sh
   cargo run --release --features wayland-support -- --watch --tray
   ```
## Contributing
[WIP]  
Contributions welcome!

## Licensing & Attributions
This project is licensed under the **GNU General Public License v3.0**.

Upon first use of the ``Kihon`` plugin, the following datasets are downloaded and are the property of their respective owners:
 - **JMdict-Simplified:** A JSON conversion of the JMdict dictionary files provided by [**scriptin/jmdict-simplified**](https://github.com/scriptin/jmdict-simplified) under the [**CC BY-SA 4.0 License**](https://creativecommons.org/licenses/by-sa/4.0/).
 - **JmdictFurigana:** Furigana data for the JMdict provided by [**Doublevil/JmdictFurigana**](https://github.com/Doublevil/JmdictFurigana) under the **MIT License**.
   - **JMdict:** The original JMdict XML files are property of the **Electronic Dictionary Research and Development Group**, used in accordance with the [**EDRDG Licence**](https://www.edrdg.org/edrdg/licence.html).

 - **Word Frequencies:** The BCCWJ SUW and BCCWJ LUW combined provided by [**Kuuuube/yomitan-dictionaries**](https://github.com/Kuuuube/yomitan-dictionaries) and the [**Jiten Frequency List**](https://jiten.moe/other).
   - **Balanced Corpus of Contemporary Written Japanese (BCCWJ):** The combined version is based on the SUW and LUW Frequency Lists by the [**National Institute for Japanese Language and Linguistics**](https://clrd.ninjal.ac.jp/bccwj/en/freq-list.html).
   - **Jiten:** The [**Jiten.moe**](https://jiten.moe/) Frequency Lists by [**Sirush**](https://github.com/Sirush), used in accordance with the [**CC BY-SA 4.0 License**](https://creativecommons.org/licenses/by-sa/4.0/). For proper version control, [**my repository**](https://github.com/jasmine-blush/jiten-moe-frequency) is used for the actual dependency download.

When using the ``Kihon`` plugin, the following is used:
 - **Vibrato:** The Vibrato tokenizer provided by [**daac-tools/vibrato**](https://github.com/daac-tools/vibrato) under the **MIT License**. For tokenization the [jumandic-mecab-7_0](https://github.com/daac-tools/vibrato/releases/download/v0.5.0/jumandic-mecab-7_0.tar.xz) file is downloaded (if not already present).

   - **JumanDIC:** The JumanDIC is the property of **Kyoto University** and is provided at [ku-nlp/JumanDIC](https://github.com/ku-nlp/JumanDIC).

The ``Jotoba`` plugin uses the API of the website [jotoba.de](https://jotoba.de/). Jotoba is an amazing multilingual Japanese dictionary website, please check it out! A big Thank You to the creators of Jotoba for their great [API implementation](https://jotoba.de/docs.html)!
