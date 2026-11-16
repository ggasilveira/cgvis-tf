//! SceneRenderer: owns the static and dynamic instance buffers and the material
//! pipelines, decides which items go to which buffer, and draws a frame in phases.
//!
//! Frame order:   draw_solid  ->  (skybox)  ->  draw_transparent
//! Bind groups:   0 = frame (camera, lights; set by the caller)
//!                1 = instance buffer (set by draw_range)
//!                2 = material (set by draw_range)
//!
//! Assumes the types from earlier: Scene, Instance, Mobility, Model, NodeKind,
//! Assets, Material, Mesh, InstanceBuffer, BufferKind, DrawItem, SortHint,
//! AlphaMode, MaterialPipelines.

use super::scene::Scene;
use crate::render::asset_manager::AssetManager;
use crate::render::bindgroup::InstanceBufferBindGroupLayout;
use crate::render::context::RenderContext;
use crate::render::instance_buffer::BufferKind;
use crate::render::material::AlphaMode;

use super::instance_buffer::InstanceBuffer;
use super::pipeline::MaterialPipelines;
use glam::Vec3;

pub struct SceneRenderer {
    pipelines: MaterialPipelines,
    static_instances: InstanceBuffer,
    dynamic_instances: InstanceBuffer,
    instance_layout: InstanceBufferBindGroupLayout,
    /// Last Scene::dynamic_version seen, to know when the scene changed.
    seen_dynamic_version: u64,
}

impl SceneRenderer {
    pub fn new(ctx: &RenderContext, pipelines: MaterialPipelines) -> Self {
        let instance_layout = InstanceBufferBindGroupLayout::new(&ctx.device);

        Self {
            pipelines,
            static_instances: InstanceBuffer::new(
                &ctx.device,
                &instance_layout,
                BufferKind::Static,
                1024,
            ),
            dynamic_instances: InstanceBuffer::new(
                &ctx.device,
                &instance_layout,
                BufferKind::Dynamic,
                256,
            ),
            instance_layout,
            seen_dynamic_version: u64::MAX, // forces the first update
        }
    }

    /// Once per level: build the static buffer. The camera doesn't matter here,
    /// because the static buffer never contains blended items.
    pub fn load_level(&mut self, ctx: &RenderContext, assets: &AssetManager, scene: &Scene) {
        self.static_instances
            .update(ctx, &self.instance_layout, scene, Vec3::ZERO, assets);
    }

    /// Each frame, before drawing.
    pub fn update(
        &mut self,
        ctx: &RenderContext,
        assets: &AssetManager,
        scene: &Scene,
        camera_pos: Vec3,
    ) {
        //let scene_changed = scene.dynamic_version() != self.seen_dynamic_version;
        // Blended items must be re-sorted whenever the camera moves, so a
        // buffer containing any is rebuilt every frame.
        //if scene_changed || self.dynamic_instances.has_blend() {
        self.dynamic_instances
            .update(ctx, &self.instance_layout, scene, camera_pos, assets);
        //self.seen_dynamic_version = scene.dynamic_version();
        //}
    }

    /// Opaque, then alpha-tested geometry from both buffers.
    /// The frame bind group (group 0) must already be set on the pass.
    pub fn draw_solid(&self, pass: &mut wgpu::RenderPass, assets: &AssetManager) {
        for mode in [AlphaMode::Opaque, AlphaMode::Mask] {
            self.static_instances
                .draw_mode(pass, assets, &self.pipelines, mode);
            self.dynamic_instances
                .draw_mode(pass, assets, &self.pipelines, mode);
        }
    }

    /// Blended geometry, back to front. Only the dynamic buffer has any.
    pub fn draw_transparent(&self, pass: &mut wgpu::RenderPass, assets: &AssetManager) {
        self.dynamic_instances
            .draw_mode(pass, assets, &self.pipelines, AlphaMode::Blend);
    }
}

// // ---------------------------------------------------------------------------
// // Scene side: a version counter instead of a dirty flag, so the renderer only
// // needs `&Scene` and several renderers (e.g. a shadow pass) can each track it.
// //
// // impl Scene {
// //     pub fn dynamic_version(&self) -> u64 { self.dynamic_version }
// //
// //     pub fn set_transform(&mut self, key: InstanceKey, transform: Mat4) {
// //         if let Some(inst) = self.instances.get_mut(key) {
// //             inst.transform = transform;
// //             self.dynamic_version += 1;
// //         }
// //     }
// // }
// //
// // Usage per frame:
// //
// // renderer.update(&ctx, &scene, &assets, camera.position);
// // pass.set_bind_group(0, &frame_bind_group, &[]);
// // renderer.draw_solid(&mut pass, &assets);
// // skybox.draw(&mut pass);
// // renderer.draw_transparent(&mut pass, &assets);
// // ---------------------------------------------------------------------------
