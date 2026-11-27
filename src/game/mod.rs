mod door;
mod player;
mod slenderman;

use crate::{engine::prelude::*, game::door::Door};
use player::Player;

pub fn setup(ctx: &mut Context) -> anyhow::Result<()> {
    ctx.load_level("levels/forest.toml")?;

    for id in ctx.find_all("door") {
        ctx.add_behaviour(id, Door::new(90.0));
    }
    ctx.spawn("player")
        .with_position(Vec3::new(0.0, 3.0, 0.0))
        .with_behaviour(Player::new());
    Ok(())
}
