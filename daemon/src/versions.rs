use rebuilderd_common::errors::*;
use rebuilderd_common::VersionCmp;
use std::process::Command;
use crate::models;
use rebuilderd_common::PkgRelease;
use std::cmp::Ordering;

// TOOD: this needs to be more configurable
pub fn cmp(distro: &VersionCmp, old: &str, new: &str) -> Result<Ordering> {
    match distro {
        VersionCmp::Basic => cmp_basic(old, new),
        VersionCmp::Debian => cmp_debian(old, new),
    }
}

pub fn cmp_basic(old: &str, new: &str) -> Result<Ordering> {
    if old != new {
        // assume versions never go backwards
        Ok(Ordering::Greater)
    } else {
        Ok(Ordering::Equal)
    }
}

pub fn cmp_debian(old: &str, new: &str) -> Result<Ordering> {
    if old == new {
        return Ok(Ordering::Equal)
    }

    trace!("Running dpkg to compare {:?} with {:?}", old, new);
    let status = Command::new("dpkg")
        .args(&[
            "--compare-versions",
            old,
            "lt",
            new,
        ])
        .status()?;

    if status.success() {
        Ok(Ordering::Greater)
    } else {
        Ok(Ordering::Less)
    }
}

pub trait PkgVerCmp {
    fn bump_package(&mut self, distro: &VersionCmp, new: &PkgRelease) -> Result<Ordering> {
        let ord = cmp(distro, self.version(), &new.version)?;
        if ord == Ordering::Greater {
            self.apply_fields(new);
        }
        Ok(ord)
    }

    fn version(&self) -> &str;

    fn apply_fields(&mut self, new: &PkgRelease);
}

impl PkgVerCmp for models::Package {
    fn version(&self) -> &str {
        &self.version
    }

    fn apply_fields(&mut self, new: &PkgRelease) {
        self.version = new.version.clone();
        self.architecture = new.architecture.clone();
        self.artifact_url = new.artifact_url.clone();
        self.input_url = new.input_url.clone();
    }
}

impl PkgVerCmp for PkgRelease {
    fn version(&self) -> &str {
        &self.version
    }

    fn apply_fields(&mut self, new: &PkgRelease) {
        self.version = new.version.clone();
        self.artifact_url = new.artifact_url.clone();
        self.input_url = new.input_url.clone();
    }
}
