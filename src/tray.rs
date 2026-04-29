use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

pub fn spawn_tray_icon(
    paused: Arc<AtomicBool>,
    ocr_model: Arc<AtomicUsize>,
    do_paste: Arc<AtomicBool>,
) {
    tracing::info!("Spawning tray icon.");

    #[cfg(target_os = "linux")]
    {
        use ksni::TrayMethods;
        std::thread::spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let tray = MyTray {
                    paused,
                    ocr_model,
                    do_paste,
                };
                let _handle = tray.spawn().await.unwrap();

                std::future::pending::<()>().await;
            });
        });
    }
    #[cfg(target_os = "windows")]
    {
        std::thread::spawn(move || {
            use tray_icon::{
                Icon, TrayIconBuilder,
                menu::{Menu, MenuEvent, MenuItem},
            };
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                DispatchMessageW, GetMessageW, MSG, TranslateMessage,
            };

            let icon_bytes = include_bytes!("./assets/icon_windows.png");
            let image = image::load_from_memory(icon_bytes)
                .expect("Failed to load tray icon")
                .to_rgba8();
            let (width, height) = image.dimensions();
            let icon = Icon::from_rgba(image.into_raw(), width, height).unwrap();

            let tray_menu = Menu::new();
            let pause_item = MenuItem::new("Pause", true, None);
            let paste_item = MenuItem::new("Paste", true, None);
            let active_ocr_model = ocr_model.load(Ordering::Relaxed);
            let ocr_label = if active_ocr_model == 0 {
                "Switch to MangaOCR"
            } else if active_ocr_model == 1 {
                "Switch to Tesseract"
            } else {
                ""
            };
            let ocr_item = MenuItem::new(ocr_label, true, None);
            let quit_item = MenuItem::new("Exit", true, None);

            tray_menu.append(&pause_item).unwrap();
            tray_menu.append(&paste_item).unwrap();
            tray_menu.append(&ocr_item).unwrap();
            tray_menu.append(&quit_item).unwrap();

            let tray = TrayIconBuilder::new()
                .with_menu(Box::new(tray_menu))
                .with_tooltip("Popup Dictionary")
                .with_icon(icon)
                .build()
                .unwrap();

            unsafe {
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
                    if let Ok(event) = MenuEvent::receiver().try_recv() {
                        if event.id == quit_item.id() {
                            std::process::exit(0);
                        } else if event.id == pause_item.id() {
                            let was_paused = paused.fetch_xor(true, Ordering::Relaxed);
                            if was_paused {
                                tracing::info!("Resuming watcher.");
                                pause_item.set_text("Pause");
                            } else {
                                tracing::info!("Pausing watcher.");
                                pause_item.set_text("Resume");
                            }
                        } else if event.id == ocr_item.id() {
                            let previous_model = ocr_model.fetch_xor(1, Ordering::Relaxed);
                            if previous_model == 0 {
                                tracing::info!("Switching to MangaOCR.");
                                ocr_item.set_text("Switch to Tesseract");
                            } else if previous_model == 1 {
                                tracing::info!("Switching to MangaOCR.");
                                ocr_item.set_text("Switch to MangaOCR");
                            }
                        } else if event.id == paste_item.id() {
                            do_paste.store(true, Ordering::Relaxed);
                            tracing::info!("Pasting from clipboard.");
                        }
                    }

                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        });
    }
}

#[derive(Debug)]
#[cfg(target_os = "linux")]
struct MyTray {
    paused: Arc<AtomicBool>,
    ocr_model: Arc<AtomicUsize>,
    do_paste: Arc<AtomicBool>,
}

#[cfg(target_os = "linux")]
impl ksni::Tray for MyTray {
    fn id(&self) -> String {
        "popup-dictionary".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let icon_bytes = include_bytes!("./assets/icon_linux_macos.png");
        let image = image::load_from_memory(icon_bytes)
            .expect("Failed to load tray icon")
            .to_rgba8();

        let (width, height) = image.dimensions();
        let pixels = image.into_raw();

        vec![ksni::Icon {
            width: width as i32,
            height: height as i32,
            data: pixels,
        }]
    }

    fn title(&self) -> String {
        "Popup Dictionary".into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let active_ocr_model = self.ocr_model.load(Ordering::Relaxed);
        let ocr_label = if active_ocr_model == 0 {
            "Switch to MangaOCR"
        } else if active_ocr_model == 1 {
            "Switch to Tesseract"
        } else {
            ""
        };
        let is_paused = self.paused.load(Ordering::Relaxed);
        let pause_label = if is_paused { "Resume" } else { "Pause" };
        let paste_label = "Paste";
        vec![
            StandardItem {
                label: pause_label.into(),
                activate: Box::new(|this: &mut Self| {
                    let was_paused = this.paused.fetch_xor(true, Ordering::Relaxed);
                    if was_paused {
                        tracing::info!("Resuming watcher.");
                    } else {
                        tracing::info!("Pausing watcher.");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: paste_label.into(),
                activate: Box::new(|this: &mut Self| {
                    this.do_paste.store(true, Ordering::Relaxed);
                    tracing::info!("Pasting from clipboard.");
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: ocr_label.into(),
                activate: Box::new(|this: &mut Self| {
                    let previous_model = this.ocr_model.fetch_xor(1, Ordering::Relaxed);
                    if previous_model == 0 {
                        tracing::info!("Switching to MangaOCR.");
                    } else {
                        tracing::info!("Switching to Tesseract.");
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Exit".into(),
                activate: Box::new(|_| {
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
