/// A portable filename stem; output directories/atomic finalization belong to
/// the real file adapter in Phase 3. Never use untrusted titles as paths.
pub fn safe_filename_stem(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| if "/\\<>:\"|?*".contains(c) { '_' } else { c })
        .take(120)
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let stem = if trimmed.is_empty() {
        "untitled"
    } else {
        trimmed
    };
    let base = stem.split('.').next().unwrap_or(stem).to_ascii_uppercase();
    if matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
    {
        format!("_{stem}")
    } else {
        stem.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_stems_cannot_traverse_or_be_windows_devices() {
        assert_eq!(safe_filename_stem("../../movie\0.mp4"), "_.._movie.mp4");
        assert_eq!(safe_filename_stem("... "), "untitled");
        assert_eq!(safe_filename_stem("CON.mp4"), "_CON.mp4");
        assert_eq!(safe_filename_stem("LPT1"), "_LPT1");
        assert_eq!(safe_filename_stem("Sunny afternoon"), "Sunny afternoon");
        assert!(safe_filename_stem("a/b\\c:d")
            .chars()
            .all(|c| !"/\\:".contains(c)));
    }
}
