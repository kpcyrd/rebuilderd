pub fn secs_to_human(duration: i64) -> String {
    let secs = duration % 60;
    let mins = duration / 60;
    let hours = mins / 60;
    let mins = mins % 60;

    let mut out = Vec::new();
    if hours > 0 {
        out.push(format!("{}h", hours));
    }
    if mins > 0 {
        out.push(format!("{}m", mins));
    }
    out.push(format!("{}s", secs));

    out.join(" ")
}
