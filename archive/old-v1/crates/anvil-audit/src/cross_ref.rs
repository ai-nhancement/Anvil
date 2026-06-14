/// A cross-reference key identifying a specific section of an artifact at a given version.
///
/// Format: `<artifact-path>:<section-id>:<version>`
/// Example: `charter.md:§introduction:v1`
///
/// All three fields must be non-empty and must not contain a colon. Artifact paths
/// must be relative (not absolute) to avoid drive-letter colons on Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRefKey {
    pub artifact_path: String,
    pub section_id: String,
    pub version: String,
}

impl CrossRefKey {
    #[must_use]
    pub fn new(artifact_path: &str, section_id: &str, version: &str) -> Self {
        Self {
            artifact_path: artifact_path.to_owned(),
            section_id: section_id.to_owned(),
            version: version.to_owned(),
        }
    }

    /// Serializes to the canonical `<artifact-path>:<section-id>:<version>` wire form.
    #[must_use]
    pub fn to_key_string(&self) -> String {
        format!(
            "{}:{}:{}",
            self.artifact_path, self.section_id, self.version
        )
    }

    /// Parses from the canonical wire form.
    ///
    /// Returns `None` if the string does not contain **exactly** two `:` separators,
    /// if any field is empty, or if any field itself contains a colon.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return None;
        }
        let (artifact_path, section_id, version) = (parts[0], parts[1], parts[2]);
        if artifact_path.is_empty() || section_id.is_empty() || version.is_empty() {
            return None;
        }
        Some(Self {
            artifact_path: artifact_path.to_owned(),
            section_id: section_id.to_owned(),
            version: version.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // hinge_test: pins=cross-ref-format, intended=cross-reference-key-format, phase=P2
    #[test]
    fn test_cross_reference_key_stability() {
        // Pins: cross-reference keys use the format <artifact-path>:<section-id>:<version>.
        // Changing this format breaks all existing cross-reference lookups in stored records.
        let key = CrossRefKey::new("charter.md", "§introduction", "v1");
        assert_eq!(key.to_key_string(), "charter.md:§introduction:v1");

        let parsed =
            CrossRefKey::parse("charter.md:§introduction:v1").expect("should parse valid key");
        assert_eq!(parsed.artifact_path, "charter.md");
        assert_eq!(parsed.section_id, "§introduction");
        assert_eq!(parsed.version, "v1");
        assert_eq!(parsed, key);

        // Round-trip stability
        let round_tripped =
            CrossRefKey::parse(&key.to_key_string()).expect("round-trip should succeed");
        assert_eq!(round_tripped, key);
    }

    #[test]
    fn test_cross_ref_key_rejects_incomplete_input() {
        assert!(CrossRefKey::parse("charter.md:§intro").is_none());
        assert!(CrossRefKey::parse("only-one-part").is_none());
        assert!(CrossRefKey::parse("::").is_none());
    }

    #[test]
    fn test_cross_ref_key_rejects_extra_colons() {
        // Extra colons must be rejected — each field must itself contain no colon.
        assert!(CrossRefKey::parse("a:b:c:d").is_none());
        assert!(CrossRefKey::parse("a:b:c:d:e").is_none());
        // The old splitn(3) behaviour would have silently folded "c:d" into version.
        // This verifies the strict 3-part requirement.
    }
}
