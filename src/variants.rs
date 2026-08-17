/// Smart Username Variant Generator for OSINT investigations.
///
/// Given a base username, generates intelligent permutations that OSINT
/// investigators would typically try manually:
///
/// - Separator insertion at camelCase/word boundaries: `krish_pranav`, `krish.pranav`
/// - Numeric suffixes/prefixes: `user1`, `user123`, `user_`, `0user`
/// - Common hacker/alt prefixes & suffixes: `xuser`, `theuser`, `realuser`, `user_official`
/// - Basic l33t speak: `us3r`, `h4cker`
/// - Underscore/dot wrapping: `_user_`, `.user.`

use std::collections::HashSet;

/// Generate smart username variants for OSINT scanning.
///
/// Returns a deduplicated, sorted list of variant usernames (excluding the
/// original, which the caller is expected to scan separately).
pub fn generate_variants(base: &str) -> Vec<String> {
    let mut variants: HashSet<String> = HashSet::new();
    let base_lower = base.to_lowercase();

    // ── 1. Separator insertion at word boundaries ──────────────────────────
    let boundaries = find_word_boundaries(base);
    for pos in &boundaries {
        for sep in &['_', '.', '-'] {
            let mut v = String::with_capacity(base.len() + 1);
            v.push_str(&base[..*pos]);
            v.push(*sep);
            v.push_str(&base[*pos..]);
            variants.insert(v);
        }
    }

    // ── 2. Numeric suffixes ───────────────────────────────────────────────
    for n in &["1", "2", "12", "123", "69", "99", "00", "01", "007"] {
        variants.insert(format!("{}{}", base, n));
        variants.insert(format!("{}_{}", base, n));
    }

    // ── 3. Numeric prefixes ───────────────────────────────────────────────
    for n in &["0", "1", "x", "xx"] {
        variants.insert(format!("{}{}", n, base));
    }

    // ── 4. Common OSINT alt-account prefixes ──────────────────────────────
    for prefix in &["the", "real", "official", "im", "its", "iam", "not", "hey"] {
        variants.insert(format!("{}{}", prefix, base));
        variants.insert(format!("{}_{}", prefix, base));
        variants.insert(format!("{}{}", prefix, &base_lower));
    }

    // ── 5. Common OSINT alt-account suffixes ──────────────────────────────
    for suffix in &["_", "__", "official", "real", "hq", "dev", "irl", "alt", "backup"] {
        variants.insert(format!("{}{}", base, suffix));
        variants.insert(format!("{}_{}", base, suffix));
    }

    // ── 6. Underscore / dot wrapping ──────────────────────────────────────
    variants.insert(format!("_{}_", base));
    variants.insert(format!("_{}",  base));
    variants.insert(format!("{}_",  base));
    variants.insert(format!(".{}.", base));
    variants.insert(format!(".{}",  base));
    variants.insert(format!("{}.",  base));

    // ── 7. Basic l33t speak ───────────────────────────────────────────────
    let leet = base_lower
        .replace('a', "4")
        .replace('e', "3")
        .replace('i', "1")
        .replace('o', "0")
        .replace('s', "5")
        .replace('t', "7");
    if leet != base_lower {
        variants.insert(leet);
    }

    // ── 8. Case variants ──────────────────────────────────────────────────
    if base != base_lower {
        variants.insert(base_lower.clone());
    }
    let upper = base.to_uppercase();
    if upper != base {
        variants.insert(upper);
    }

    // Remove the original so it isn't scanned twice
    variants.remove(base);
    // Also remove empty strings
    variants.remove("");

    let mut result: Vec<String> = variants.into_iter().collect();
    result.sort();
    result
}

/// Find likely word-boundary positions in a username string.
///
/// Detects camelCase transitions (`aB`) and letter-digit transitions (`a1`, `1a`).
fn find_word_boundaries(s: &str) -> Vec<usize> {
    let chars: Vec<char> = s.chars().collect();
    let mut positions = Vec::new();

    for i in 1..chars.len() {
        let prev = chars[i - 1];
        let curr = chars[i];

        // camelCase: lowercase followed by uppercase
        if prev.is_lowercase() && curr.is_uppercase() {
            positions.push(i);
        }
        // letter→digit or digit→letter
        if prev.is_alphabetic() && curr.is_numeric() {
            positions.push(i);
        }
        if prev.is_numeric() && curr.is_alphabetic() {
            positions.push(i);
        }
    }

    positions.dedup();
    positions
}

/// Print a summary of the generated variants to stdout.
pub fn print_variant_summary(base: &str, variants: &[String], no_color: bool) {
    use colored::Colorize;

    if no_color {
        println!("\n[*] Variant Generator: {} variants for \"{}\"", variants.len(), base);
        println!("───────────────────────────────────────");
        for (i, v) in variants.iter().enumerate() {
            println!("  {:>3}. {}", i + 1, v);
        }
        println!("───────────────────────────────────────\n");
    } else {
        println!(
            "\n{} {} {} variants for \"{}\"",
            "🧬".to_string(),
            "Variant Generator:".bright_magenta().bold(),
            variants.len().to_string().bright_white().bold(),
            base.bright_cyan()
        );
        println!(
            "{}",
            "───────────────────────────────────────".bright_magenta()
        );
        for (i, v) in variants.iter().enumerate() {
            println!(
                "  {} {}",
                format!("{:>3}.", i + 1).dimmed(),
                v.bright_yellow()
            );
        }
        println!(
            "{}\n",
            "───────────────────────────────────────".bright_magenta()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_variants() {
        let variants = generate_variants("krishPranav");
        assert!(!variants.is_empty());
        // Should contain camelCase splits
        assert!(variants.contains(&"krish_Pranav".to_string()));
        assert!(variants.contains(&"krish.Pranav".to_string()));
        // Should not contain the original
        assert!(!variants.contains(&"krishPranav".to_string()));
    }

    #[test]
    fn numeric_suffixes() {
        let variants = generate_variants("user");
        assert!(variants.contains(&"user1".to_string()));
        assert!(variants.contains(&"user123".to_string()));
        assert!(variants.contains(&"user_123".to_string()));
    }

    #[test]
    fn leet_speak() {
        let variants = generate_variants("hacker");
        assert!(variants.contains(&"h4ck3r".to_string()));
    }

    #[test]
    fn alt_prefixes() {
        let variants = generate_variants("blue");
        assert!(variants.contains(&"theblue".to_string()));
        assert!(variants.contains(&"real_blue".to_string()));
    }
}
