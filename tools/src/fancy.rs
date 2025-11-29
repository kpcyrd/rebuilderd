use colored::Colorize;
use rebuilderd_common::api::v1::{ArtifactStatus, BuildStatus};

pub trait Fancy {
    fn fancy(&self) -> String;
}

impl Fancy for BuildStatus {
    fn fancy(&self) -> String {
        match self {
            BuildStatus::Good => format!("{:5}", self.as_str().green()),
            BuildStatus::Bad => format!("{:5}", self.as_str().red()),
            BuildStatus::Fail => format!("{:5}", self.as_str().red()),
            BuildStatus::Unknown => format!("{:5}", self.as_str().yellow()),
        }
    }
}

impl Fancy for ArtifactStatus {
    fn fancy(&self) -> String {
        match self {
            ArtifactStatus::Good => format!("{:5}", self.as_str().green()),
            ArtifactStatus::Bad => format!("{:5}", self.as_str().red()),
            ArtifactStatus::Unknown => format!("{:5}", self.as_str().yellow()),
        }
    }
}
