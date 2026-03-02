use std::f64::consts::TAU;
use std::sync::mpsc::{Receiver, Sender, channel};

use dioxus_native::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use galileo::Color;
use galileo::control::{EventProcessor, MapController, MouseButton, RawUserEvent};
use galileo::galileo_types::cartesian::{Point2, Size};
use galileo::galileo_types::geo::impls::GeoPoint2d;
use galileo::galileo_types::geo::{Crs, NewGeoPoint};
use galileo::galileo_types::impls::{ClosedContour, Polygon};
use galileo::layer::feature_layer::FeatureLayer;
use galileo::layer::feature_layer::symbol::SimplePolygonSymbol;
use galileo::layer::raster_tile_layer::RasterTileLayerBuilder;
use galileo::render::WgpuRenderer;
use galileo::{Map, MapBuilder};
use rand::RngExt;
use wgpu::{
    Device, Extent3d, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureViewDescriptor,
};

const MAP_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

const PARIS_LAT: f64 = 48.8566;
const PARIS_LON: f64 = 2.3522;

pub enum MapEvent {
    PointerMoved(f64, f64),
    ButtonPressed(MapMouseButton),
    ButtonReleased(MapMouseButton),
    Scroll(f64),
}

#[derive(Debug, Copy, Clone)]
pub enum MapMouseButton {
    Left,
    Right,
    Middle,
}

struct TextureAndHandle {
    texture: Texture,
    handle: TextureHandle,
}

struct ActiveRenderer {
    device: Device,
    renderer: WgpuRenderer,
    event_processor: EventProcessor,
    displayed_texture: Option<TextureAndHandle>,
    next_texture: Option<TextureAndHandle>,
}

pub struct MapPaintSource {
    map: Map,
    active: Option<ActiveRenderer>,
    rx: Receiver<MapEvent>,
    tx: Sender<MapEvent>,
    last_size: (u32, u32),
}

impl MapPaintSource {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let map = create_map();

        Self {
            map,
            active: None,
            rx,
            tx,
            last_size: (0, 0),
        }
    }

    pub fn sender(&self) -> Sender<MapEvent> {
        self.tx.clone()
    }

    fn process_events(&mut self) {
        let Some(active) = &mut self.active else {
            return;
        };
        while let Ok(event) = self.rx.try_recv() {
            let raw = match event {
                MapEvent::PointerMoved(x, y) => RawUserEvent::PointerMoved(Point2::new(x, y)),
                MapEvent::ButtonPressed(b) => RawUserEvent::ButtonPressed(convert_button(b)),
                MapEvent::ButtonReleased(b) => RawUserEvent::ButtonReleased(convert_button(b)),
                MapEvent::Scroll(d) => RawUserEvent::Scroll(d),
            };
            active.event_processor.handle(raw, &mut self.map);
        }
    }
}

impl CustomPaintSource for MapPaintSource {
    fn resume(&mut self, device_handle: &DeviceHandle) {
        let device = device_handle.device.clone();
        let queue = device_handle.queue.clone();

        let renderer = WgpuRenderer::new_with_device_and_texture(
            device.clone(),
            queue.clone(),
            Size::new(1, 1),
        );

        let mut event_processor = EventProcessor::default();
        event_processor.add_handler(MapController::default());

        self.active = Some(ActiveRenderer {
            device,
            renderer,
            event_processor,
            displayed_texture: None,
            next_texture: None,
        });
    }

    fn suspend(&mut self) {
        self.active = None;
    }

    fn render(
        &mut self,
        mut ctx: CustomPaintCtx<'_>,
        width: u32,
        height: u32,
        scale: f64,
    ) -> Option<TextureHandle> {
        if width == 0 || height == 0 {
            return None;
        }

        self.process_events();
        self.map.animate();

        let active = self.active.as_mut()?;

        if self.last_size != (width, height) {
            self.last_size = (width, height);
            active.renderer.resize(Size::new(width, height));
            let logical_w = width as f64 / scale;
            let logical_h = height as f64 / scale;
            self.map.set_size(Size::new(logical_w, logical_h));

            if let Some(tex) = active.next_texture.take() {
                ctx.unregister_texture(tex.handle);
            }
            if let Some(tex) = active.displayed_texture.take() {
                ctx.unregister_texture(tex.handle);
            }
        }

        if active.next_texture.is_none() {
            let texture = create_texture(&active.device, width, height);
            let handle = ctx.register_texture(texture.clone());
            active.next_texture = Some(TextureAndHandle { texture, handle });
        }

        self.map.load_layers();

        let next = active.next_texture.as_ref().unwrap();
        let view = next.texture.create_view(&TextureViewDescriptor::default());
        active.renderer.render_to_texture_view(&self.map, &view);

        let handle = next.handle.clone();

        std::mem::swap(&mut active.next_texture, &mut active.displayed_texture);

        Some(handle)
    }
}

fn convert_button(btn: MapMouseButton) -> MouseButton {
    match btn {
        MapMouseButton::Left => MouseButton::Left,
        MapMouseButton::Right => MouseButton::Right,
        MapMouseButton::Middle => MouseButton::Middle,
    }
}

fn make_hex_polygon(lat: f64, lon: f64) -> Polygon<GeoPoint2d> {
    let radius = 0.5;
    let points: Vec<GeoPoint2d> = (0..6)
        .map(|i| {
            let theta = (i as f64) * TAU / 6.0;
            NewGeoPoint::latlon(lat + radius * theta.cos(), lon + radius * theta.sin())
        })
        .collect();

    Polygon::new(ClosedContour::new(points), vec![])
}

fn create_map() -> Map {
    let raster_layer = RasterTileLayerBuilder::new_osm()
        .with_file_cache_checked(".tile_cache")
        .build()
        .expect("failed to create OSM tile layer");

    let mut rng = rand::rng();
    let hexes: Vec<Polygon<GeoPoint2d>> = (0..1000)
        .into_iter()
        .map(|_| {
            let lat = rng.random_range(-90.0..90.0);
            let lon = rng.random_range(-180.0..180.0);
            make_hex_polygon(lat, lon)
        })
        .collect();

    let symbol = SimplePolygonSymbol::new(Color::rgba(255, 100, 50, 120))
        .with_stroke_color(Color::rgba(255, 50, 0, 255))
        .with_stroke_width(3.0);
    let polygon_layer = FeatureLayer::new(hexes, symbol, Crs::WGS84);

    MapBuilder::default()
        .with_latlon(PARIS_LAT, PARIS_LON)
        .with_z_level(8)
        .with_layer(raster_layer)
        .with_layer(polygon_layer)
        .build()
}

fn create_texture(device: &Device, width: u32, height: u32) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some("galileo_map_texture"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: MAP_TEXTURE_FORMAT,
        usage: TextureUsages::RENDER_ATTACHMENT
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}
