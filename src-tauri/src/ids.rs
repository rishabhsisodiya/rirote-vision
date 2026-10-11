//! Stable names for things kept on disk.

/// A small hash that is the same in every release of the app; the standard
/// library's is not. Used to name the files kept for a project folder.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The name under which a project folder's files are kept.
pub fn project_key(project: &std::path::Path) -> String {
    format!("{:016x}", fnv1a(project.to_string_lossy().as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hash_is_fixed() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(project_key(std::path::Path::new("")), "cbf29ce484222325");
    }
}
