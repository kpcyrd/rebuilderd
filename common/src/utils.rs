pub fn secs_to_human(duration: i64) -> String {
    let secs = duration % 60;
    let mins = duration / 60;
    let hours = mins / 60;
    let mins = mins % 60;

    let mut out = Vec::new();
    if hours > 0 {
        out.push(format!("{:2}h", hours));
    }
    if mins > 0 || hours > 0 {
        out.push(format!("{:2}m", mins));
    }
    out.push(format!("{:2}s", secs));

    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secs_to_human_0s() {
        let x = secs_to_human(0);
        assert_eq!(x, " 0s");
    }

    #[test]
    fn test_secs_to_human_1s() {
        let x = secs_to_human(1);
        assert_eq!(x, " 1s");
    }

    #[test]
    fn test_secs_to_human_1m() {
        let x = secs_to_human(60);
        assert_eq!(x, " 1m  0s");
    }

    #[test]
    fn test_secs_to_human_1m_30s() {
        let x = secs_to_human(90);
        assert_eq!(x, " 1m 30s");
    }

    #[test]
    fn test_secs_to_human_10m_30s() {
        let x = secs_to_human(630);
        assert_eq!(x, "10m 30s");
    }

    #[test]
    fn test_secs_to_human_1h() {
        let x = secs_to_human(3600);
        assert_eq!(x, " 1h  0m  0s");
    }

    #[test]
    fn test_secs_to_human_12h_10m_30s() {
        let x = secs_to_human(3600 * 12 + 600 + 30);
        assert_eq!(x, "12h 10m 30s");
    }

    #[test]
    fn test_secs_to_human_100h() {
        let x = secs_to_human(3600 * 100);
        assert_eq!(x, "100h  0m  0s");
    }
}
