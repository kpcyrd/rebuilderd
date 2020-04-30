use rebuilderd_common::errors::*;
use std::env;
use std::fs;

pub fn run(name: &str) -> Result<()> {
    let work_dir = format!("/var/lib/rebuilderd-worker/{}", name);

    info!("switching into work directory: {}", work_dir);
    fs::create_dir_all(&work_dir)?;
    env::set_current_dir(&work_dir)?;

    Ok(())
}
