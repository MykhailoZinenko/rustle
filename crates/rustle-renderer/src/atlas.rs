use std::collections::HashMap;
use serde::Deserialize;

const ATLAS_JSON: &str = include_str!("assets/atlas-msdf-v3-20260323.json");
const ATLAS_PNG: &[u8] = include_bytes!("assets/atlas-msdf-v3-20260323.png");

#[derive(Deserialize)]
struct RawAtlas {
    atlas: RawAtlasMeta,
    variants: Vec<RawVariant>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAtlasMeta {
    distance_range: f64,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct RawVariant {
    glyphs: Vec<RawGlyph>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGlyph {
    unicode: u32,
    advance: f64,
    plane_bounds: Option<RawBounds>,
    atlas_bounds: Option<RawBounds>,
}

#[derive(Deserialize)]
struct RawBounds {
    left: f64,
    bottom: f64,
    right: f64,
    top: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
}

#[derive(Debug, Clone)]
pub struct GlyphInfo {
    pub advance: f32,
    pub plane_bounds: Option<Bounds>,
    pub atlas_bounds: Option<Bounds>,
}

/// CPU-side atlas data: glyph metrics and atlas dimensions.
/// No GPU resources — safe to create in tests without a device.
pub struct AtlasData {
    pub width: u32,
    pub height: u32,
    pub distance_range: f32,
    pub glyphs: HashMap<char, GlyphInfo>,
}

impl AtlasData {
    fn parse() -> Self {
        let raw: RawAtlas = serde_json::from_str(ATLAS_JSON)
            .expect("failed to parse MSDF atlas JSON");

        let mut glyphs = HashMap::new();
        if let Some(variant) = raw.variants.first() {
            for g in &variant.glyphs {
                let ch = char::from_u32(g.unicode).unwrap_or('?');
                glyphs.insert(ch, GlyphInfo {
                    advance: g.advance as f32,
                    plane_bounds: g.plane_bounds.as_ref().map(|b| Bounds {
                        left: b.left as f32,
                        bottom: b.bottom as f32,
                        right: b.right as f32,
                        top: b.top as f32,
                    }),
                    atlas_bounds: g.atlas_bounds.as_ref().map(|b| Bounds {
                        left: b.left as f32,
                        bottom: b.bottom as f32,
                        right: b.right as f32,
                        top: b.top as f32,
                    }),
                });
            }
        }

        Self {
            width: raw.atlas.width,
            height: raw.atlas.height,
            distance_range: raw.atlas.distance_range as f32,
            glyphs,
        }
    }
}

/// Full atlas with GPU resources. Created at renderer init.
pub struct MsdfAtlas {
    pub data: AtlasData,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl MsdfAtlas {
    /// # Panics
    /// Panics if the embedded atlas JSON or PNG is corrupt (compile-time assets).
    pub fn load(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let data = AtlasData::parse();

        let img = image::load_from_memory(ATLAS_PNG)
            .expect("failed to decode MSDF atlas PNG")
            .to_rgba8();

        let tex_size = wgpu::Extent3d {
            width: data.width,
            height: data.height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("msdf_atlas_texture"),
            size: tex_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * data.width),
                rows_per_image: Some(data.height),
            },
            tex_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("msdf_atlas_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            data,
            texture,
            view,
            sampler,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_json_parses_successfully() {
        let data = AtlasData::parse();
        assert_eq!(data.width, 2048);
        assert_eq!(data.height, 2048);
        assert_eq!(data.distance_range, 16.0);
    }

    #[test]
    fn atlas_has_common_ascii_chars() {
        let data = AtlasData::parse();
        for ch in ['A', 'Z', 'a', 'z', '0', '9', ' ', '.', ','] {
            assert!(data.glyphs.contains_key(&ch), "missing glyph for '{ch}' (U+{:04X})", ch as u32);
        }
    }

    #[test]
    fn atlas_space_has_advance_but_no_bounds() {
        let data = AtlasData::parse();
        let space = data.glyphs.get(&' ').expect("missing space glyph");
        assert!(space.advance > 0.0);
        assert!(space.plane_bounds.is_none());
        assert!(space.atlas_bounds.is_none());
    }

    #[test]
    fn atlas_letter_a_has_bounds() {
        let data = AtlasData::parse();
        let a = data.glyphs.get(&'A').expect("missing 'A' glyph");
        assert!(a.advance > 0.0);
        assert!(a.plane_bounds.is_some());
        assert!(a.atlas_bounds.is_some());

        let pb = a.plane_bounds.unwrap();
        assert!(pb.right > pb.left, "plane bounds width should be positive");
        assert!(pb.top > pb.bottom, "plane bounds height should be positive");

        let ab = a.atlas_bounds.unwrap();
        assert!(ab.right > ab.left, "atlas bounds width should be positive");
        assert!(ab.top > ab.bottom, "atlas bounds height should be positive");
    }

    #[test]
    fn atlas_uniform_advance() {
        let data = AtlasData::parse();
        let expected_advance = 0.6_f32;
        for (ch, glyph) in &data.glyphs {
            assert!(
                (glyph.advance - expected_advance).abs() < 0.01,
                "glyph '{ch}' has unexpected advance {}, expected ~{expected_advance}",
                glyph.advance
            );
        }
    }

    #[test]
    fn atlas_bounds_within_texture() {
        let data = AtlasData::parse();
        let w = data.width as f32;
        let h = data.height as f32;
        for (ch, glyph) in &data.glyphs {
            if let Some(ab) = &glyph.atlas_bounds {
                assert!(ab.left >= 0.0 && ab.left <= w, "glyph '{ch}' atlas left {}", ab.left);
                assert!(ab.right >= 0.0 && ab.right <= w, "glyph '{ch}' atlas right {}", ab.right);
                assert!(ab.bottom >= 0.0 && ab.bottom <= h, "glyph '{ch}' atlas bottom {}", ab.bottom);
                assert!(ab.top >= 0.0 && ab.top <= h, "glyph '{ch}' atlas top {}", ab.top);
            }
        }
    }

    #[test]
    fn atlas_glyph_count_reasonable() {
        let data = AtlasData::parse();
        assert!(data.glyphs.len() >= 70, "expected >=70 glyphs, got {}", data.glyphs.len());
    }
}
