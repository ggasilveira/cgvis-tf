//! Serde structs for prefab files (prefabs/*.toml) and level files (level.toml).
//!
//! Cargo.toml:
//!   serde = { version = "1", features = ["derive"] }
//!   toml = "0.8"
//!   glam = "0.29"

use glam::{Mat4, Quat, Vec3};
use serde::Deserialize;

// ===========================================================================
// Prefab
//
// model = "models/door.glb"
// mobility = "dynamic"
// colliders = "model"            # use the COL_ nodes in the .glb
//
// [[collider]]                   # ...or define them here (not both)
// shape = "capsule"
// radius = 0.35
// height = 8.0
// offset = [0.0, 4.0, 0.0]
//
// [interact]
// kind = "door"
// open_angle = 90.0
// sound = "sounds/door_creak.ogg"
// ===========================================================================

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)] // typos like `mobilty` become load errors
pub struct PrefabDef {
    pub model: String,

    #[serde(default)]
    pub mobility: Mobility,

    /// `colliders = "model"`: take colliders from the .glb's collider nodes.
    #[serde(default)]
    pub colliders: Option<ColliderSource>,

    /// `[[collider]]` tables: colliders defined in the prefab itself.
    #[serde(default, rename = "collider")]
    pub collider_defs: Vec<ColliderDef>,

    #[serde(default)]
    pub interact: Option<Interact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mobility {
    #[default]
    Static,
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColliderSource {
    Model,
}

#[derive(Debug, Deserialize)]
pub struct ColliderDef {
    #[serde(flatten)]
    pub shape: ShapeDef,
    #[serde(default)]
    pub offset: [f32; 3],
    #[serde(default = "identity_rotation")]
    pub rotation: [f32; 4], // x, y, z, w
}

/// `shape = "..."` picks the variant; the other keys are its fields.
#[derive(Debug, Deserialize)]
#[serde(tag = "shape", rename_all = "lowercase")]
pub enum ShapeDef {
    Box { half_extents: [f32; 3] },
    Sphere { radius: f32 },
    Capsule { radius: f32, height: f32 },
    Cylinder { radius: f32, height: f32 },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Interact {
    Door {
        open_angle: f32,
        #[serde(default)]
        sound: Option<String>,
    },
    Page,
}

impl PrefabDef {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let def: Self = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        def.validate()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(def)
    }

    /// Rules serde can't express on its own.
    fn validate(&self) -> Result<(), String> {
        if self.colliders.is_some() && !self.collider_defs.is_empty() {
            return Err(
                "use either `colliders = \"model\"` or [[collider]] tables, not both".into(),
            );
        }
        Ok(())
    }
}

// ===========================================================================
// Level
//
// [[instance]]
// prefab = "pine"
// position = [12.5, 0.0, -3.2]
// rotation = [0.0, 0.38, 0.0, 0.92]   # optional, default identity
// scale = [1.05, 1.05, 1.05]          # optional, default 1
//
// [instance.overrides]                # optional
// mobility = "dynamic"
// ===========================================================================

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelDef {
    #[serde(default, rename = "instance")]
    pub instances: Vec<InstanceDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceDef {
    pub prefab: String,
    pub position: [f32; 3],
    #[serde(default = "identity_rotation")]
    pub rotation: [f32; 4], // x, y, z, w
    #[serde(default = "unit_scale")]
    pub scale: [f32; 3],
    #[serde(default)]
    pub overrides: Option<Overrides>,
}

/// Per-instance changes merged over the prefab at spawn time.
/// Every field is optional: None means "use the prefab's value".
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overrides {
    pub mobility: Option<Mobility>,
}

impl InstanceDef {
    pub fn transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::from_array(self.scale),
            Quat::from_array(self.rotation).normalize(), // x, y, z, w
            Vec3::from_array(self.position),
        )
    }

    pub fn mobility(&self, prefab: &PrefabDef) -> Mobility {
        self.overrides
            .as_ref()
            .and_then(|o| o.mobility)
            .unwrap_or(prefab.mobility)
    }
}

impl LevelDef {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }
}

// ---------------------------------------------------------------------------
// serde defaults
// ---------------------------------------------------------------------------

fn identity_rotation() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn unit_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn one_u32() -> u32 {
    1
}
