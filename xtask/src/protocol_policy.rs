//! Generate allocation-guard metadata from the same file descriptor as prost.

use prost_types::field_descriptor_proto::{Label, Type};
use prost_types::{DescriptorProto, EnumDescriptorProto, FileDescriptorSet};
use std::collections::BTreeMap;
use std::fmt::Write as _;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn append(descriptor: &FileDescriptorSet, output: &mut String) -> Result<()> {
    let file = descriptor.file.first().ok_or("missing descriptor")?;
    let package = format!(".{}", file.package());
    let mut messages = Vec::new();
    collect(&package, &file.message_type, &mut messages, 0)?;
    let enums = enum_metadata(&package, &file.enum_type)?;
    for (constant, name) in [
        ("CLIENT_ROOT", "ClientEnvelope"),
        ("SERVER_ROOT", "ServerEnvelope"),
    ] {
        let index = messages
            .iter()
            .position(|(full, _)| full == &format!("{package}.{name}"))
            .ok_or("missing root message")?;
        writeln!(output, "pub(crate) const {constant}: usize = {index};")?;
    }
    output.push_str("pub(crate) const GUARD_POLICIES: &[crate::preflight::Policy] = &[\n");
    for (full, message) in &messages {
        let real_unions = message
            .field
            .iter()
            .filter(|field| !field.proto3_optional())
            .filter_map(|field| field.oneof_index)
            .collect::<std::collections::BTreeSet<_>>();
        if message.field.len() > 32 || !message.enum_type.is_empty() || real_unions.len() > 1 {
            return Err("new descriptor shape requires guard review".into());
        }
        let closed = message
            .field
            .iter()
            .any(|f| f.oneof_index.is_some() && !f.proto3_optional());
        writeln!(
            output,
            "crate::preflight::Policy {{ closed: {closed}, fields: &["
        )?;
        for field in &message.field {
            let kind = match Type::try_from(field.r#type.ok_or("missing field type")?)? {
                Type::Uint32 => "U32".to_owned(),
                Type::Uint64 => "U64".to_owned(),
                Type::Sint32 => "S32".to_owned(),
                Type::Bool => "Bool".to_owned(),
                Type::String => format!(
                    "String({})",
                    match (full.as_str(), field.name()) {
                        (".tether.control.v1.ClientHello", "build_identifier") => 64,
                        (".tether.control.v1.ClientHello", "client_display_name") => 128,
                        _ => 4096,
                    }
                ),
                Type::Bytes => "Bytes(16)".to_owned(),
                Type::Message => format!(
                    "Message({})",
                    messages
                        .iter()
                        .position(|(name, _)| name == field.type_name())
                        .ok_or("unknown message type")?
                ),
                Type::Enum => {
                    let enumeration = enums.get(field.type_name()).ok_or("unknown enum type")?;
                    let values = enumeration
                        .value
                        .iter()
                        .map(|v| v.number().to_string())
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("Enum(&[{values}])")
                }
                _ => return Err("new scalar type requires guard review".into()),
            };
            let repeated = if field.label() == Label::Repeated {
                if field.r#type() == Type::Enum {
                    enums
                        .get(field.type_name())
                        .ok_or("unknown enum")?
                        .value
                        .iter()
                        .filter(|v| v.number() != 0)
                        .count()
                } else if field.r#type() == Type::Message {
                    256
                } else {
                    return Err("new repeated type requires guard review".into());
                }
            } else {
                0
            };
            let union = field.oneof_index.is_some() && !field.proto3_optional();
            writeln!(
                output,
                "crate::preflight::Field {{ number: {}, kind: crate::preflight::Kind::{kind}, repeated: {repeated}, union: {union} }},",
                field.number()
            )?;
        }
        output.push_str("]},\n");
    }
    output.push_str("];\n");
    Ok(())
}

fn enum_metadata<'a>(
    package: &str,
    enums: &'a [EnumDescriptorProto],
) -> Result<BTreeMap<String, &'a EnumDescriptorProto>> {
    let mut result = BTreeMap::new();
    for enumeration in enums {
        if enumeration.value.iter().any(|v| v.number() < 0) {
            return Err("negative enum definitions require guard review".into());
        }
        result.insert(format!("{package}.{}", enumeration.name()), enumeration);
    }
    Ok(result)
}

fn collect<'a>(
    prefix: &str,
    messages: &'a [DescriptorProto],
    out: &mut Vec<(String, &'a DescriptorProto)>,
    depth: usize,
) -> Result<()> {
    if depth > 8 || out.len() + messages.len() > 128 {
        return Err("descriptor metadata exceeds guard bounds".into());
    }
    for message in messages {
        let full = format!("{prefix}.{}", message.name());
        out.push((full.clone(), message));
        collect(&full, &message.nested_type, out, depth + 1)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost_types::{
        EnumDescriptorProto, EnumValueDescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    };

    fn descriptor() -> FileDescriptorSet {
        FileDescriptorSet {
            file: vec![FileDescriptorProto {
                package: Some("test".into()),
                message_type: vec![
                    DescriptorProto {
                        name: Some("ClientEnvelope".into()),
                        ..Default::default()
                    },
                    DescriptorProto {
                        name: Some("ServerEnvelope".into()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
        }
    }

    #[test]
    fn metadata_is_derived_and_new_unreviewed_shapes_fail_closed() {
        let mut value = descriptor();
        let mut output = String::new();
        append(&value, &mut output).unwrap();
        assert!(output.contains("CLIENT_ROOT: usize = 0"));
        assert!(output.contains("SERVER_ROOT: usize = 1"));
        value.file[0].message_type[0]
            .field
            .push(FieldDescriptorProto {
                number: Some(1),
                r#type: Some(Type::Float.into()),
                ..Default::default()
            });
        assert!(append(&value, &mut String::new()).is_err());
        value = descriptor();
        value.file[0].enum_type.push(EnumDescriptorProto {
            name: Some("Negative".into()),
            value: vec![EnumValueDescriptorProto {
                number: Some(-1),
                ..Default::default()
            }],
            ..Default::default()
        });
        assert!(append(&value, &mut String::new()).is_err());
        value = descriptor();
        value.file[0].message_type[0].field = (0..33)
            .map(|number| FieldDescriptorProto {
                number: Some(number),
                r#type: Some(Type::Uint32.into()),
                ..Default::default()
            })
            .collect();
        assert!(append(&value, &mut String::new()).is_err());
    }

    #[test]
    fn optional_presence_is_not_a_real_union_but_multiple_real_unions_need_review() {
        let mut value = descriptor();
        value.file[0].message_type[0].field = vec![FieldDescriptorProto {
            number: Some(1),
            r#type: Some(Type::Uint32.into()),
            oneof_index: Some(0),
            proto3_optional: Some(true),
            ..Default::default()
        }];
        let mut output = String::new();
        append(&value, &mut output).unwrap();
        assert!(output.contains("closed: false"));
        assert!(!output.contains("union: true"));
        value.file[0].message_type[0].field[0].proto3_optional = Some(false);
        value.file[0].message_type[0]
            .field
            .push(FieldDescriptorProto {
                number: Some(2),
                r#type: Some(Type::Uint32.into()),
                oneof_index: Some(1),
                ..Default::default()
            });
        assert!(append(&value, &mut String::new()).is_err());
    }
}
