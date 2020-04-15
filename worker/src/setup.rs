use rebuilderd_common::errors::*;
use std::env;
use std::fs;

pub fn run(name: &str) -> Result<()> {
    let config_path = format!("/etc/rebuilderd-worker/{}", name);
    let work_directory = format!("/var/lib/rebuilderd-worker/{}", name);

    info!("writing worker default config to {}", config_path);
    fs::create_dir_all(&format!("{}/archlinux-repro", config_path))?;
    fs::write(&format!("{}/archlinux-repro/repro.conf", config_path), format!("BUILDDIRECTORY={}\n", work_directory))?;

    let work_directory = format!("/var/lib/rebuilderd-worker/{}", name);
    info!("switching into work directory: {}", work_directory);
    fs::create_dir_all(&work_directory)?;
    env::set_current_dir(&work_directory)?;
    env::set_var("XDG_CONFIG_HOME", work_directory);

    Ok(())
}
