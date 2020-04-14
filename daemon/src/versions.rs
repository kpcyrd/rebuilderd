use rebuilderd_common::errors::*;
use rebuilderd_common::{Distro, Status};
use std::process::Command;
use crate::models;
use rebuilderd_common::PkgRelease;
use std::cmp::Ordering;

pub fn cmp(distro: &Distro, old: &str, new: &str) -> Result<Ordering> {
    match distro {
        Distro::Archlinux => cmp_archlinux(old, new),
        Distro::Debian => cmp_debian(old, new),
    }
}

pub fn cmp_archlinux(old: &str, new: &str) -> Result<Ordering> {
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
    fn bump_package(&mut self, distro: &Distro, new: &PkgRelease) -> Result<Ordering> {
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
        // TODO: make sure we updated all fields necessary
        self.status = Status::Unknown.to_string();
        self.version = new.version.clone();
        self.url = new.url.clone();
    }
}

impl PkgVerCmp for PkgRelease {
    fn version(&self) -> &str {
        &self.version
    }

    fn apply_fields(&mut self, new: &PkgRelease) {
        // TODO: make sure we updated all fields necessary
        self.status = Status::Unknown;
        self.version = new.version.clone();
        self.url = new.url.clone();
    }
}
