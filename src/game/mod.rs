mod player;
mod slenderman;

use crate::engine::prelude::*;
use player::Player;

pub fn setup(ctx: &mut Context) -> anyhow::Result<()> {
    ctx.spawn_prefab("tree1", Transform::IDENTITY)?;
    ctx.spawn("player")
        .with_position(Vec3::new(0.0, 1.7, -8.0))
        .with_behaviour(Player::new());
    Ok(())
}
