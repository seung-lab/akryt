use serde::Deserialize;
use std::time;

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub pipe: Pipe,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Pipe {
    pub name: String,
    pub source: Source,
    pub destination: Destination,
    pub spillway: Spillway,
    pub threads: Threads,
    pub encodings: Vec<EncodingRule>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub path: String,
    #[serde(deserialize_with = "parse_byte_size")]
    pub size: u64,
    #[serde(with = "humantime_serde")]
    pub poll: std::time::Duration,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Destination {
    pub path: String,
    #[serde(rename = "secrets-file")]
    pub secrets_file: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Spillway {
    pub path: String,
    #[serde(deserialize_with = "parse_byte_size")]
    pub size: u64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Threads {
    pub worker: usize,
    pub codec: usize,
}

fn default_effort() -> u8 { 3 }

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Transcode {
    pub extension: String,
    pub format: String,
    pub colorspace: String,
    pub quality: f32,
    #[serde(default = "default_effort")]
    pub effort: u8,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Compress {
    pub extension: String,
    pub compress: String,
    pub level: u8,    
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(tag = "action")]
pub enum EncodingRule {
    #[serde(rename = "compress")]
    Compress(Compress),
    #[serde(rename = "transcode")]
    Transcode(Transcode),
}

// PARSING

fn parse_byte_size<'de, D>(d: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    parse_byte_size_str(&s).map_err(serde::de::Error::custom)
}

fn parse_byte_size_str(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let i = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());
    let (num, unit) = (&s[..i], &s[i..]);
    let n: u64 = num.parse().map_err(|_| format!("bad number: {num}"))?;
    
    let (mult, label) = match unit.to_ascii_lowercase().as_str() {
        "" | "b"  => (1u64, "B"),
        "k" | "kb"   => (1_000, "KB"),
        "ki" | "kib" => (1_024, "KiB"),
        "m" | "mb"   => (1_000_000, "MB"),
        "mi" | "mib" => (1_048_576, "MiB"),
        "g" | "gb"   => (1_000_000_000, "GB"),
        "gi" | "gib" => (1_073_741_824, "GiB"),
        other => return Err(format!("unknown unit: {other:?}")),
    };

    n.checked_mul(mult)
        .ok_or_else(|| format!("{n}{label} overflows u64"))
}
