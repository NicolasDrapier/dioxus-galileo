mod map_paint_source;

use std::any::Any;

use dioxus::prelude::*;
use dioxus_native::use_wgpu;
use winit::dpi::LogicalSize;
use winit::window::WindowAttributes;

use crate::map_paint_source::{MapEvent, MapMouseButton, MapPaintSource};

static MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    env_logger::init();

    let window_attrs = WindowAttributes::default()
        .with_title("Carte Galileo")
        .with_inner_size(LogicalSize::new(1024, 768));

    let config: Vec<Box<dyn Any>> = vec![Box::new(window_attrs)];

    dioxus_native::launch_cfg(App, Vec::new(), config);
}

#[component]
pub fn App() -> Element {
    rsx! {
        document::Stylesheet { href: MAIN_CSS }

        main { style: "width: 100%; height: 100%; margin: 0; padding: 0;",
            div { width: "100vw", height: "100vh", MapWidget {} }
        }
    }
}

#[component]
pub fn MapWidget() -> Element {
    let paint_source = MapPaintSource::new();
    let sender = paint_source.sender();
    let paint_source_id = use_wgpu(move || paint_source);

    let sender = use_hook(move || sender);

    let s_move = sender.clone();
    let s_down = sender.clone();
    let s_up = sender.clone();
    let s_wheel = sender.clone();

    rsx! {
        div {
            id: "canvas-container",
            style: "width: 100%; height: 100%; margin: 0; padding: 0; background: #000; overflow: hidden;",

            onmousemove: move |evt: Event<MouseData>| {
                let coords = evt.data().client_coordinates();
                let _ = s_move.send(MapEvent::PointerMoved(coords.x, coords.y));
            },

            onmousedown: move |evt: Event<MouseData>| {
                let coords = evt.data().client_coordinates();
                let btn = convert_dioxus_button(evt.data().trigger_button());
                let _ = s_down.send(MapEvent::PointerMoved(coords.x, coords.y));
                let _ = s_down.send(MapEvent::ButtonPressed(btn));
            },

            onmouseup: move |evt: Event<MouseData>| {
                let coords = evt.data().client_coordinates();
                let btn = convert_dioxus_button(evt.data().trigger_button());
                let _ = s_up.send(MapEvent::PointerMoved(coords.x, coords.y));
                let _ = s_up.send(MapEvent::ButtonReleased(btn));
            },

            onwheel: move |evt: Event<WheelData>| {
                let raw = evt.data().delta().strip_units();
                let scroll = -raw.y / 120.0;
                if scroll.abs() > 0.0001 {
                    let _ = s_wheel.send(MapEvent::Scroll(scroll));
                }
            },

            canvas {
                id: "video-canvas",
                "src": paint_source_id,
                style: "width: 100%; height: 100%; display: block;",
            }
        }
    }
}

fn convert_dioxus_button(btn: Option<dioxus::html::input_data::MouseButton>) -> MapMouseButton {
    use dioxus::html::input_data::MouseButton;
    match btn {
        Some(MouseButton::Primary) => MapMouseButton::Left,
        Some(MouseButton::Secondary) => MapMouseButton::Right,
        Some(MouseButton::Auxiliary) => MapMouseButton::Middle,
        _ => MapMouseButton::Left,
    }
}
