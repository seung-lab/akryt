use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub pipe: Pipe,
}

#[derive(Debug, Deserialize)]
pub struct Pipe {
    pub name: String,
    pub source: Source,
    pub destination: Destination,
    pub spillway: Spillway,
    pub threads: Threads,
    pub encodings: Vec<EncodingRule>,
}

#[derive(Debug, Deserialize)]
pub struct Source {
    pub path: String,
    pub size: String
}

#[derive(Debug, Deserialize)]
pub struct Destination {
    pub path: String,
    #[serde(rename = "secrets-file")]
    pub secrets_file: String,
}

#[derive(Debug, Deserialize)]
pub struct Spillway {
    pub path: String,
    pub size: String,
}


#[derive(Debug, Deserialize)]
pub struct Threads {
    pub worker: u32,
    pub codec: u32,
}

fn default_effort() -> u8 { 3 }

#[derive(Debug, Deserialize)]
pub struct Transcode {
    pub extension: String,
    pub format: String,
    pub colorspace: String,
    pub quality: f32,
    #[serde(default = "default_effort")]
    pub effort: u8,
}

#[derive(Debug, Deserialize)]
pub struct Compress {
    pub extension: String,
    pub compress: String,
    pub level: u8,    
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action")]
pub enum EncodingRule {
    #[serde(rename = "compress")]
    Compress(Compress),
    #[serde(rename = "transcode")]
    Transcode(Transcode),
}