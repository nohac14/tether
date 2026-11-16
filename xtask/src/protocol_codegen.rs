//! Offline descriptor-to-Rust generation. Never parses runtime control messages.

use prost::Message;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

type GenerationResult<T> = Result<T, Box<dyn std::error::Error>>;

pub fn generate(descriptor: &Path, output: &Path) -> GenerationResult<()> {
    let metadata = fs::symlink_metadata(descriptor)?;
    if !metadata.is_file() || metadata.len() > 1_048_576 {
        return Err("invalid generation descriptor type/size".into());
    }
    let bytes = fs::read(descriptor)?;
    let descriptors = prost_types::FileDescriptorSet::decode(bytes.as_slice())?;
    // Deliberately small, explicit schema boundary. Changes require review.
    if descriptors.file.len() != 1
        || descriptors.file[0].name.as_deref() != Some("tether/control/v1/control.proto")
        || descriptors.file[0].package.as_deref() != Some("tether.control.v1")
    {
        return Err("unexpected generation descriptor files/package".into());
    }
    if !fs::symlink_metadata(output)?.is_dir() || fs::read_dir(output)?.next().is_some() {
        return Err("generation output must be an empty real directory".into());
    }
    let mut config = prost_build::Config::new();
    config.out_dir(output).skip_debug(["."]);
    // Owner-approved 2026-11-14: independent wire capability flags are not a
    // mutually exclusive state machine. No module/crate-wide lint waiver.
    config.type_attribute(
        ".tether.control.v1.CapabilitySnapshot",
        "#[expect(clippy::struct_excessive_bools, reason = \"Approved independent protobuf capability flags\")]",
    );
    // Buf provides the descriptor; this does not find/download/execute protoc.
    config.compile_fds(descriptors.clone())?;
    let path = output.join("tether.control.v1.rs");
    let mut source = normalize_generated(&fs::read_to_string(&path)?);
    append_redacted_debug(&mut source)?;
    crate::protocol_policy::append(&descriptors, &mut source)?;
    fs::write(path, source)?;
    Ok(())
}

// Preserve schema prose and improve its Rustdoc formatting; retain strict
// handwritten/generated lint checks rather than disabling documentation lints.
fn normalize_generated(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    for line in source.lines() {
        if line.trim_start().starts_with("///") {
            let mut quoted = false;
            for part in line.split_inclusive(|character: char| {
                !character.is_ascii_alphanumeric() && character != '_'
            }) {
                let word = part.trim_end_matches(|character: char| {
                    !character.is_ascii_alphanumeric() && character != '_'
                });
                let suffix = &part[word.len()..];
                let mut lowercase_seen = false;
                let camel_case = word.chars().any(|character| {
                    if character.is_ascii_lowercase() {
                        lowercase_seen = true;
                    }
                    lowercase_seen && character.is_ascii_uppercase()
                });
                if camel_case && !quoted {
                    result.push('`');
                    result.push_str(word);
                    result.push('`');
                } else {
                    result.push_str(word);
                }
                result.push_str(suffix);
                if suffix.contains('`') {
                    quoted = !quoted;
                }
            }
        } else {
            if line.trim_start().starts_with("pub fn as_str_name(")
                || line.trim_start().starts_with("pub fn from_str_name(")
            {
                result.push_str("#[must_use]\n");
            }
            result.push_str(line);
        }
        result.push('\n');
    }
    result
}

fn append_redacted_debug(source: &mut String) -> GenerationResult<()> {
    let parsed = syn::parse_file(source)?;
    source.push_str("\n// Tether generator: all wire diagnostics are unconditionally redacted.\n");
    append_items(&parsed.items, "", 0, source)
}

fn append_items(
    items: &[syn::Item],
    prefix: &str,
    depth: usize,
    source: &mut String,
) -> GenerationResult<()> {
    if depth > 8 || items.len() > 512 {
        return Err("generated Rust type tree exceeds reviewed bounds".into());
    }
    for item in items {
        let (name, generics) = match item {
            syn::Item::Struct(item) => (&item.ident, &item.generics),
            syn::Item::Enum(item) => (&item.ident, &item.generics),
            syn::Item::Mod(module) => {
                let Some((_, nested)) = &module.content else {
                    return Err("unexpected external generated module".into());
                };
                append_items(
                    nested,
                    &format!("{prefix}{}::", module.ident),
                    depth + 1,
                    source,
                )?;
                continue;
            }
            _ => continue,
        };
        if !generics.params.is_empty() || generics.where_clause.is_some() {
            return Err("unexpected generic generated type".into());
        }
        writeln!(
            source,
            "impl ::core::fmt::Debug for {prefix}{name} {{
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {{
                    f.write_str(\"WireMessage(<redacted>)\")
                }}
            }}"
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_messages_enums_nested_oneofs_and_raw_identifiers() {
        let mut source = "pub struct TextInsertion { pub text: String }\n\
            pub enum MouseButton { Left, Right }\n\
            pub mod r#type { pub enum Action { Text(String) } }\n"
            .to_owned();
        append_redacted_debug(&mut source).unwrap();
        for name in ["TextInsertion", "MouseButton", "r#type::Action"] {
            assert!(source.contains(&format!("impl ::core::fmt::Debug for {name}")));
        }
        assert_eq!(source.matches("WireMessage(<redacted>)").count(), 3);
        assert!(!source.contains("self.text"));
        assert!(!source.contains("match self"));
        syn::parse_file(&source).unwrap();
    }

    #[test]
    fn rejects_unexpected_syntax_instead_of_silently_omitting_redaction() {
        for text in [
            "pub struct Generic<T>(T);",
            "pub mod external;",
            "not Rust!",
        ] {
            assert!(append_redacted_debug(&mut text.to_owned()).is_err());
        }
    }

    #[test]
    fn generated_documentation_and_enum_helpers_retain_strict_lint_contract() {
        let input = "/// ClientEnvelope and `ProtoBuf` names.\n\
            pub fn as_str_name(&self) -> &'static str { \"X\" }\n\
            pub fn from_str_name(_: &str) -> Option<Self> { None }\n";
        let output = normalize_generated(input);
        assert!(output.contains("/// `ClientEnvelope` and `ProtoBuf` names."));
        assert_eq!(output.matches("#[must_use]").count(), 2);
        assert!(!output.contains("allow("));
    }
}
