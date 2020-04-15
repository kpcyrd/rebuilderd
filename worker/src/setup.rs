use rebuilderd_common::errors::*;
use std::env;
use std::fs;

pub fn run(name: &str) -> Result<()> {
    let config_dir = format!("/etc/rebuilderd-worker/{}", name);
    let work_dir = format!("/var/lib/rebuilderd-worker/{}", name);
    let repro_work_dir = format!("{}/repro", work_dir);

    info!("writing worker default config to {}", config_dir);
    fs::create_dir_all(&format!("{}/archlinux-repro", config_dir))?;
    fs::write(&format!("{}/archlinux-repro/repro.conf", config_dir), format!("BUILDDIRECTORY={}\n", repro_work_dir))?;

    info!("switching into work directory: {}", work_dir);
    fs::create_dir_all(&repro_work_dir)?;
    env::set_current_dir(&work_dir)?;
    env::set_var("XDG_CONFIG_HOME", config_dir);

    Ok(())
}
