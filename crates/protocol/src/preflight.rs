//! Allocation-free structural guard driven by descriptor-generated metadata.

use crate::validation::{Limits, ValidationError};

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    U32,
    U64,
    S32,
    Bool,
    Enum(&'static [i32]),
    String(usize),
    Bytes(usize),
    Message(usize),
}

pub(crate) struct Field {
    pub number: u32,
    pub kind: Kind,
    // Zero means singular, otherwise the cardinality ceiling.
    pub repeated: usize,
    pub union: bool,
}

pub(crate) struct Policy {
    pub fields: &'static [Field],
    pub closed: bool,
}

pub(crate) fn check(bytes: &[u8], root: usize, limits: Limits) -> Result<(), ValidationError> {
    if bytes.len() > limits.message_bytes() {
        return Err(ValidationError::MessageTooLarge);
    }
    message(bytes, root, 0, &mut 4096, limits)
}

fn message(
    mut bytes: &[u8],
    index: usize,
    depth: usize,
    remaining: &mut usize,
    limits: Limits,
) -> Result<(), ValidationError> {
    let policy = crate::wire::GUARD_POLICIES
        .get(index)
        .ok_or(ValidationError::InvalidContext)?;
    if depth > 16 || policy.fields.len() > 32 {
        return Err(ValidationError::MalformedMessage);
    }
    let mut counts = [0_usize; 32];
    let mut union_seen = false;
    while !bytes.is_empty() {
        visit(remaining)?;
        let tag = varint(&mut bytes)?;
        let number = u32::try_from(tag >> 3).map_err(|_| ValidationError::MalformedMessage)?;
        if number == 0 || number > 0x1fff_ffff {
            return Err(ValidationError::MalformedMessage);
        }
        let wire_type = tag & 7;
        let Some((slot, field)) = policy
            .fields
            .iter()
            .enumerate()
            .find(|(_, f)| f.number == number)
        else {
            if policy.closed {
                return Err(ValidationError::Unsupported);
            }
            skip(&mut bytes, wire_type)?;
            continue;
        };
        if field.union {
            if union_seen {
                return Err(ValidationError::MalformedMessage);
            }
            union_seen = true;
        }
        let count = if field.repeated > 0 && wire_type == 2 && matches!(field.kind, Kind::Enum(_)) {
            let mut packed = delimited(&mut bytes)?;
            let mut count = 0;
            while !packed.is_empty() {
                visit(remaining)?;
                scalar(field.kind, varint(&mut packed)?)?;
                count += 1;
                if count + counts[slot] > field.repeated {
                    return Err(ValidationError::LimitExceeded);
                }
            }
            count
        } else {
            match field.kind {
                Kind::Message(child) if wire_type == 2 => {
                    message(delimited(&mut bytes)?, child, depth + 1, remaining, limits)?;
                }
                Kind::String(maximum) if wire_type == 2 => {
                    let value = delimited(&mut bytes)?;
                    if value.len() > maximum {
                        return Err(ValidationError::LimitExceeded);
                    }
                    std::str::from_utf8(value).map_err(|_| ValidationError::MalformedMessage)?;
                }
                Kind::Bytes(maximum) if wire_type == 2 => {
                    if delimited(&mut bytes)?.len() > maximum {
                        return Err(ValidationError::LimitExceeded);
                    }
                }
                Kind::U32 | Kind::U64 | Kind::S32 | Kind::Bool | Kind::Enum(_)
                    if wire_type == 0 =>
                {
                    scalar(field.kind, varint(&mut bytes)?)?;
                }
                _ => return Err(ValidationError::MalformedMessage),
            }
            1
        };
        counts[slot] += count;
        let maximum = if field.repeated == 0 {
            1
        } else if matches!(field.kind, Kind::Message(_)) {
            field.repeated.min(limits.operations())
        } else {
            field.repeated
        };
        if counts[slot] > maximum {
            return Err(if field.repeated == 0 {
                ValidationError::MalformedMessage
            } else {
                ValidationError::LimitExceeded
            });
        }
    }
    Ok(())
}

fn visit(remaining: &mut usize) -> Result<(), ValidationError> {
    *remaining = remaining
        .checked_sub(1)
        .ok_or(ValidationError::LimitExceeded)?;
    Ok(())
}

fn scalar(kind: Kind, value: u64) -> Result<(), ValidationError> {
    match kind {
        Kind::U64 => Ok(()),
        Kind::U32 | Kind::S32 if u32::try_from(value).is_ok() => Ok(()),
        Kind::Bool if value <= 1 => Ok(()),
        Kind::Enum(values) if i32::try_from(value).is_ok_and(|v| values.contains(&v)) => Ok(()),
        Kind::Enum(_) => Err(ValidationError::Unsupported),
        _ => Err(ValidationError::MalformedMessage),
    }
}

fn varint(bytes: &mut &[u8]) -> Result<u64, ValidationError> {
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let (&byte, rest) = bytes
            .split_first()
            .ok_or(ValidationError::MalformedMessage)?;
        *bytes = rest;
        if shift == 63 && byte > 1 {
            return Err(ValidationError::MalformedMessage);
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(ValidationError::MalformedMessage)
}

fn delimited<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], ValidationError> {
    let length = usize::try_from(varint(bytes)?).map_err(|_| ValidationError::MalformedMessage)?;
    let value = bytes
        .get(..length)
        .ok_or(ValidationError::MalformedMessage)?;
    *bytes = &bytes[length..];
    Ok(value)
}

fn skip(bytes: &mut &[u8], wire_type: u64) -> Result<(), ValidationError> {
    let length = match wire_type {
        0 => {
            varint(bytes)?;
            return Ok(());
        }
        1 => 8,
        2 => {
            delimited(bytes)?;
            return Ok(());
        }
        5 => 4,
        _ => return Err(ValidationError::MalformedMessage),
    };
    *bytes = bytes
        .get(length..)
        .ok_or(ValidationError::MalformedMessage)?;
    Ok(())
}
