#[cfg(feature = "hyprland-support")]
use hyprland::dispatch::{Dispatch, DispatchType, Position, WindowIdentifier};
#[cfg(feature = "hyprland-support")]
use hyprland::prelude::*;

use egui::Pos2;
use enigo::{Enigo, Mouse, Settings};
use std::error::Error;
use x11rb::{
    connection::Connection,
    protocol::xproto::{AtomEnum, ConfigureWindowAux, ConnectionExt, Window},
    rust_connection::RustConnection,
};

use crate::app::APP_NAME;

pub fn get_optimal_init_pos(
    #[cfg(feature = "hyprland-support")] is_hyprland: bool,
    width: f32,
    height: f32,
) -> Result<Pos2, Box<dyn Error>> {
    let mut cursor_pos: Option<Pos2> = None;
    let mut display_size: Option<Pos2> = None;
    'outer: {
        #[cfg(feature = "hyprland-support")]
        if is_hyprland {
            use hyprland::data::{CursorPosition, Monitor};

            if let Ok(pos) = CursorPosition::get() {
                cursor_pos = Some(Pos2::new(pos.x as f32, pos.y as f32));
            }
            if let Ok(monitor) = Monitor::get_active() {
                display_size = Some(Pos2::new(
                    (monitor.width as i32 + monitor.x) as f32,
                    (monitor.height as i32 + monitor.y) as f32,
                ));
            }

            if cursor_pos.is_some() && display_size.is_some() {
                break 'outer;
            }
        }

        // try x11/windows/macos
        // wayland unlikely to work
        // this can report wrong values, so making sure not to overwrite previous good values
        let enigo: Enigo = Enigo::new(&Settings::default())?;
        if cursor_pos.is_none() {
            if let Ok((x, y)) = enigo.location() {
                cursor_pos = Some(Pos2::new(x as f32, y as f32));
            }
        }
        if display_size.is_none() {
            if let Ok((x, y)) = enigo.main_display() {
                display_size = Some(Pos2::new(x as f32, y as f32));
            }
        }
    }

    if let Some(cursor_pos) = cursor_pos
        && let Some(display_size) = display_size
    {
        tracing::debug!(
            "Fetched cursor position as x: {}, y: {} and display size as width: {}, height: {}",
            cursor_pos.x,
            cursor_pos.y,
            display_size.x,
            display_size.y
        );

        if display_size.x >= cursor_pos.x && display_size.y >= cursor_pos.y {
            let mut window_x: f32 = cursor_pos.x;
            let mut window_y: f32 = cursor_pos.y;

            if window_x + width > display_size.x {
                window_x -= width;
            }

            if window_y + height > display_size.y {
                window_y -= height;
            }

            return Ok(Pos2::new(window_x, window_y));
        } else {
            return Err(Box::from(format!(
                "Cursor position x: {}, y: {} outside display bounds width: {}, height: {}.",
                cursor_pos.x, cursor_pos.y, display_size.x, display_size.y
            )));
        }
    } else {
        return Err(Box::from(
            "No valid cursor position and/or display size found.",
        ));
    }
}

pub fn move_window_x11(x: i32, y: i32) -> Result<(), Box<dyn Error>> {
    tracing::info!("Trying to move window on x11.");
    let (connection, display_idx) = RustConnection::connect(None)?;
    let display = &connection.setup().roots[display_idx];

    match find_window_by_title_x11(&connection, display.root, APP_NAME)? {
        Some(window) => {
            tracing::debug!("Found window: 0x{:x}", window);
            configure_window_pos_x11(&connection, window, x, y)?;
            tracing::debug!("Moved window to position x: {}, y: {})", x, y);
        }
        None => {
            return Err(Box::from("Window not found."));
        }
    }

    Ok(())
}

fn find_window_by_title_x11(
    connection: &RustConnection,
    root: Window,
    title: &str,
) -> Result<Option<Window>, Box<dyn Error>> {
    let tree = connection.query_tree(root)?.reply()?;

    for &child in &tree.children {
        let window_title = get_window_title_x11(connection, child)?;
        if window_title.contains(title) {
            return Ok(Some(child));
        }

        if let Some(found) = find_window_by_title_x11(connection, child, title)? {
            return Ok(Some(found));
        }
    }

    Ok(None)
}

fn get_window_title_x11(
    connection: &RustConnection,
    window: Window,
) -> Result<String, Box<dyn Error>> {
    let net_wm_name = connection
        .intern_atom(false, b"_NET_WM_NAME")?
        .reply()?
        .atom;
    let utf8_string = connection.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;

    if let Ok(reply) = connection
        .get_property(false, window, net_wm_name, utf8_string, 0, 1024)?
        .reply()
    {
        if !reply.value.is_empty() {
            return Ok(String::from_utf8_lossy(&reply.value).into_owned());
        }
    }

    // Fallback to WM_NAME
    if let Ok(reply) = connection
        .get_property(false, window, AtomEnum::WM_NAME, AtomEnum::STRING, 0, 1024)?
        .reply()
    {
        if !reply.value.is_empty() {
            return Ok(String::from_utf8_lossy(&reply.value).into_owned());
        }
    }

    Ok(String::new())
}

fn configure_window_pos_x11(
    connection: &RustConnection,
    window: Window,
    x: i32,
    y: i32,
) -> Result<(), Box<dyn Error>> {
    let values = ConfigureWindowAux::new().x(x).y(y);

    connection.configure_window(window, &values)?;
    connection.flush()?;

    Ok(())
}

#[cfg(feature = "hyprland-support")]
pub fn move_window_hyprland(x: i16, y: i16) -> Result<(), Box<dyn Error>> {
    tracing::info!("Trying to move window on hyprland.");

    let window_id: WindowIdentifier<'_> = WindowIdentifier::Title(APP_NAME);
    /*Dispatch::call(DispatchType::ResizeWindowPixel(
        Position::Exact(WINDOW_INIT_HEIGHT, WINDOW_INIT_HEIGHT),
        window_id.to_owned(),
    ))?;*/
    Dispatch::call(DispatchType::MoveWindowPixel(
        Position::Exact(x, y),
        window_id,
    ))?;

    Ok(())
}
