mod player;
mod slenderman;

use crate::engine::prelude::*;
use player::Player;

pub fn setup(ctx: &mut Context) -> anyhow::Result<()> {
    ctx.load_level("levels/forest.toml")?;
    ctx.spawn("player")
        .with_position(Vec3::new(0.0, 3.0, 0.0))
        .with_behaviour(Player::new());
    Ok(())
}
