use dioxus_native::{CustomPaintCtx, CustomPaintSource, DeviceHandle, TextureHandle};
use galileo::galileo_types::cartesian::Size;
use galileo::layer::raster_tile_layer::RasterTileLayerBuilder;
use galileo::render::WgpuRenderer;
use galileo::{Map, MapBuilder};
use wgpu::{
    Device, Extent3d, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    TextureViewDescriptor,
};

/// Texture format matching Galileo's internal `TARGET_TEXTURE_FORMAT`.
/// Must match for MSAA resolve compatibility.
const MAP_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

struct TextureAndHandle {
    texture: Texture,
    handle: TextureHandle,
}

struct ActiveRenderer {
    device: Device,
    renderer: WgpuRenderer,
    displayed_texture: Option<TextureAndHandle>,
    next_texture: Option<TextureAndHandle>,
}

pub struct MapPaintSource {
    map: Map,
    active: Option<ActiveRenderer>,
    last_size: (u32, u32),
}

impl MapPaintSource {
    pub fn new() -> Self {
        let map = create_map();

        Self {
            map,
            active: None,
            last_size: (0, 0),
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

        self.active = Some(ActiveRenderer {
            device,
            renderer,
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
            log::warn!("render called with 0 size ({width}x{height}) — canvas has no dimensions");
            return None;
        }

        log::trace!("render {width}x{height} scale={scale}");

        self.map.animate();

        let active = self.active.as_mut()?;

        // Resize renderer + map when canvas dimensions change
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

        // Ensure we have a render target texture
        if active.next_texture.is_none() {
            let texture = create_texture(&active.device, width, height);
            let handle = ctx.register_texture(texture.clone());
            active.next_texture = Some(TextureAndHandle { texture, handle });
        }

        // Trigger async tile loading (required for tiles to appear)
        self.map.load_layers();

        // Render into our texture
        let next = active.next_texture.as_ref().unwrap();
        let view = next.texture.create_view(&TextureViewDescriptor::default());
        active.renderer.render_to_texture_view(&self.map, &view);

        let handle = next.handle.clone();

        // Double-buffer swap
        std::mem::swap(&mut active.next_texture, &mut active.displayed_texture);

        Some(handle)
    }
}

fn create_map() -> Map {
    let raster_layer = RasterTileLayerBuilder::new_osm()
        .with_file_cache_checked(".tile_cache")
        .build()
        .expect("failed to create OSM tile layer");

    MapBuilder::default()
        .with_latlon(48.866667, 2.333333)
        .with_z_level(8)
        .with_layer(raster_layer)
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
