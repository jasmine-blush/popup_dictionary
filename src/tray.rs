pub fn spawn_tray_icon() {
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
                let tray = MyTray;
                let _handle = tray.spawn().await.unwrap();

                std::future::pending::<()>().await;
            });
        });
    }
    #[cfg(target_os = "windows")]
    {
        std::thread::spawn(|| {
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
            let quit_item = MenuItem::new("Exit", true, None);
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
struct MyTray;

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
        vec![
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
