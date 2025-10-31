use colored::Colorize;
use rebuilderd_common::api::v1::{ArtifactStatus, BuildStatus};

pub trait Fancy {
    fn fancy(&self) -> String;
}

impl Fancy for BuildStatus {
    fn fancy(&self) -> String {
        match self {
            BuildStatus::Good => self.to_string().green().to_string(),
            BuildStatus::Bad => self.to_string().red().to_string(),
            BuildStatus::Fail => self.to_string().red().to_string(),
            BuildStatus::Unknown => self.to_string().yellow().to_string(),
        }
    }
}

impl Fancy for ArtifactStatus {
    fn fancy(&self) -> String {
        match self {
            ArtifactStatus::Good => self.to_string().green().to_string(),
            ArtifactStatus::Bad => self.to_string().red().to_string(),
            ArtifactStatus::Unknown => self.to_string().yellow().to_string(),
        }
    }
}
