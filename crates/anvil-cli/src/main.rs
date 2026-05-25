pub(crate) const BINARY_NAME: &str = "anvil";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{} {}", BINARY_NAME, env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }
    eprintln!(
        "{} {}: no subcommand (try --version)",
        BINARY_NAME,
        env!("CARGO_PKG_VERSION")
    );
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::BINARY_NAME;

    // hinge_test: pins=1.80, intended=stable-floor, phase=P0
    #[test]
    fn test_rust_toolchain_version_floor() {
        // Pins: rust-toolchain.toml must set channel = "stable" exactly (floor ≥1.80).
        // Flipping requires updating rust-toolchain.toml and this annotation together.
        // Checks the key=value line, not just any occurrence of "stable" in comments.
        let toolchain = include_str!("../../../rust-toolchain.toml");
        assert!(
            toolchain
                .lines()
                .any(|l| l.trim() == r#"channel = "stable""#),
            r#"rust-toolchain.toml must contain exactly: channel = "stable" (floor: ≥1.80)"#
        );
    }

    // hinge_test: pins=anvil, intended=binary-entry-point, phase=P0
    #[test]
    fn test_cli_entry_point_exists() {
        // Pins: the CLI binary is named "anvil" (the [[bin]] name in Cargo.toml).
        // Flipping requires changing BINARY_NAME and the [[bin]] declaration together.
        assert_eq!(BINARY_NAME, "anvil");
    }
}
