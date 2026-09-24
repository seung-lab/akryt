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
#[serde(tag = "action")]
pub enum EncodingRule {
    #[serde(rename = "compress")]
    Compress {
        extension: String,
        compress: String,
        level: u8,
    },
    #[serde(rename = "transcode")]
    Transcode {
        extension: String,
        format: String,
        colorspace: String,
        quality: u8,
        #[serde(default = "default_effort")]
        effort: u8,
    },
}