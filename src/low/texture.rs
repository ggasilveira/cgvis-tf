use crate::low::context::RenderContext;
use anyhow::*;
use image::GenericImageView;

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}
pub struct CreateTextureArgs {
    pub repeat_mode_vertical: wgpu::AddressMode,
    pub repeat_mode_horizontal: wgpu::AddressMode,
    pub format: wgpu::TextureFormat,
}
impl Default for CreateTextureArgs {
    fn default() -> Self {
        Self {
            repeat_mode_horizontal: wgpu::AddressMode::ClampToEdge,
            repeat_mode_vertical: wgpu::AddressMode::ClampToEdge,
            format: Texture::DEFAULT_FORMAT,
        }
    }
}

impl Texture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    pub const DEFAULT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
    pub const NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    pub const DEFAULT_ARGS: CreateTextureArgs = CreateTextureArgs {
        repeat_mode_horizontal: wgpu::AddressMode::ClampToEdge,
        repeat_mode_vertical: wgpu::AddressMode::ClampToEdge,
        format: Self::DEFAULT_FORMAT,
    };
    pub const NORMAL_ARGS: CreateTextureArgs = CreateTextureArgs {
        repeat_mode_horizontal: wgpu::AddressMode::ClampToEdge,
        repeat_mode_vertical: wgpu::AddressMode::ClampToEdge,
        format: Self::NORMAL_FORMAT,
    };

    pub fn create_depth_texture(ctx: &RenderContext, label: &str) -> Self {
        let size = wgpu::Extent3d {
            width: ctx.config.width.max(1),
            height: ctx.config.height.max(1),
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = ctx.device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });
        Self {
            texture,
            view,
            sampler,
        }
    }
    pub fn from_bytes(
        ctx: &RenderContext,
        bytes: &[u8],
        args: CreateTextureArgs,
        label: Option<&str>,
    ) -> Result<Self> {
        let img = image::load_from_memory(bytes)?;
        Self::from_image(ctx, &img, args, label)
    }
    pub fn from_image(
        ctx: &RenderContext,
        img: &image::DynamicImage,
        args: CreateTextureArgs,
        label: Option<&str>,
    ) -> Result<Self> {
        let rgba = img.to_rgba8();
        let dimensions = img.dimensions();
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: args.format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label,
            view_formats: &[],
        });
        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: args.repeat_mode_horizontal,
            address_mode_v: args.repeat_mode_vertical,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        Ok(Self {
            texture,
            view,
            sampler,
        })
    }
}
