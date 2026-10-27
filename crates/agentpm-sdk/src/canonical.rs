//! RFC 8785/JCS for new cryptographic objects only. Legacy signatures keep their
//! historical serializers. Callers normalize schema-defined arrays/timestamps first.
use anyhow::{Result, ensure};
use chrono::{DateTime, Datelike, SecondsFormat, Timelike, Utc};
use serde_json::Value;

pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Canonicalize a JSON value, including ordinary manifest floating-point values.
/// AgentPM restricts integer inputs to the interoperable range used by both libraries.
pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>> {
    validate_numbers(value, false)?;
    Ok(serde_jcs::to_vec(value)?)
}

/// Release/signature/key-set contracts deliberately contain no floating-point values.
pub fn contract_bytes(value: &Value) -> Result<Vec<u8>> {
    validate_numbers(value, true)?;
    Ok(serde_jcs::to_vec(value)?)
}

fn validate_numbers(value: &Value, integer_only: bool) -> Result<()> {
    match value {
        Value::Number(number) => {
            if let Some(number) = number.as_u64() {
                ensure!(
                    number <= MAX_SAFE_INTEGER,
                    "integer exceeds the interoperable JCS range"
                );
            } else if let Some(number) = number.as_i64() {
                ensure!(
                    number >= -(MAX_SAFE_INTEGER as i64),
                    "integer exceeds the interoperable JCS range"
                );
            } else {
                ensure!(
                    !integer_only,
                    "floating-point values are not allowed in release/signature/key-set contracts"
                );
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_numbers(value, integer_only)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                validate_numbers(value, integer_only)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Normalize new statement timestamps before JCS; never rewrite legacy timestamps.
/// Preserve nanoseconds, trim redundant fractional zeroes, and require an explicit offset.
pub fn normalize_timestamp(value: &str) -> Result<String> {
    ensure!(
        value
            .as_bytes()
            .get(10)
            .is_some_and(|b| matches!(b, b'T' | b't')),
        "timestamp must use the RFC3339 date/time separator"
    );
    let parsed = DateTime::parse_from_rfc3339(value)?;
    let timestamp = parsed.with_timezone(&Utc);
    ensure!(
        (1..=9999).contains(&parsed.year()) && (1..=9999).contains(&timestamp.year()),
        "timestamp is outside the four-digit year range"
    );
    ensure!(
        timestamp.nanosecond() < 1_000_000_000,
        "leap seconds are not supported"
    );
    // Keep the same RFC3339 grammar as the server (four-digit years and <=9 fraction digits).
    let time = value.get(19..).unwrap_or("");
    if let Some(fraction) = time.strip_prefix('.') {
        ensure!(
            fraction.bytes().take_while(u8::is_ascii_digit).count() <= 9,
            "timestamp precision exceeds nanoseconds"
        );
    }
    let seconds = timestamp.to_rfc3339_opts(SecondsFormat::Secs, true);
    ensure!(
        seconds.len() == 20,
        "timestamp is outside the four-digit year range"
    );
    let nanos = timestamp.nanosecond();
    if nanos == 0 {
        return Ok(seconds);
    }
    let fraction = format!("{nanos:09}");
    Ok(format!(
        "{}.{}Z",
        &seconds[..19],
        fraction.trim_end_matches('0')
    ))
}

#[cfg(test)]
mod tests;
