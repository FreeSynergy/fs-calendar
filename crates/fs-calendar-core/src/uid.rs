//! The identity a `uid` stands for.
//!
//! Pattern: Strategy (pure function) — one canonical text per identity, used when indexing, when
//! looking up and when comparing, so the two sides always agree.

/// The same UUID however it is written (with or without `urn:uuid:`, any case, with or without
/// hyphens) gives one text; any other `uid` stays as it is, trimmed.
pub fn normalise_uid(uid: &str) -> String {
    let t = uid.trim();
    let bare = match t.get(..9) {
        Some(head) if head.eq_ignore_ascii_case("urn:uuid:") => &t[9..],
        _ => t,
    };
    let hex: String = bare.chars().filter(|c| *c != '-').collect();
    let hyphens_fit = bare.len() == 32
        || (bare.len() == 36 && [8, 13, 18, 23].iter().all(|i| bare.as_bytes()[*i] == b'-'));
    if !hyphens_fit || hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return t.to_string();
    }
    let hex = hex.to_ascii_lowercase();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

#[cfg(test)]
mod tests {
    use super::normalise_uid;

    #[test]
    fn every_spelling_of_a_uuid_gives_one_text() {
        let plain = "0192f6a0-7c3e-7b1e-8a3d-5f6c7d8e9f01";
        for s in [
            plain,
            "urn:uuid:0192f6a0-7c3e-7b1e-8a3d-5f6c7d8e9f01",
            "URN:UUID:0192F6A0-7C3E-7B1E-8A3D-5F6C7D8E9F01",
            "0192f6a07c3e7b1e8a3d5f6c7d8e9f01",
            " 0192F6A0-7C3E-7B1E-8A3D-5F6C7D8E9F01 ",
        ] {
            assert_eq!(normalise_uid(s), plain, "{s}");
        }
        assert_eq!(
            normalise_uid(&normalise_uid("urn:uuid:ABC")),
            "urn:uuid:ABC"
        );
        assert_eq!(normalise_uid(" Plain-UID "), "Plain-UID");
    }
}
