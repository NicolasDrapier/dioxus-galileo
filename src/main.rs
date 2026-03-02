mod map_paint_source;

use std::any::Any;

use dioxus::prelude::*;
use dioxus_native::use_wgpu;
use winit::dpi::LogicalSize;
use winit::window::WindowAttributes;

use crate::map_paint_source::MapPaintSource;

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
    let paint_source_id = use_wgpu(move || paint_source);

    rsx! {
        div {
            id: "canvas-container",
            style: "width: 100%; height: 100%; margin: 0; padding: 0; background: #000; overflow: hidden;",
            canvas {
                id: "video-canvas",
                "src": paint_source_id,
                style: "width: 100%; height: 100%; display: block;",
            }
        }
    }
}
