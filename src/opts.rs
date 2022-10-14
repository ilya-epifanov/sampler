use std::{path::PathBuf, convert::TryFrom, fs::File};

use clap::Parser;
use serde::Deserialize;

#[derive(Parser)]
pub struct Opts {
    #[clap(short = 'f', long, default_value = "sampler.yaml")]
    config: PathBuf,
}

#[derive(Deserialize)]
pub struct Config {
    pub mqtt: MqttConfig,
    pub device: Option<String>,
}

fn default_client_id() -> String {
    "sampler".to_owned()
}

fn default_host() -> String {
    "localhost".to_owned()
}

fn default_port() -> u16 {
    1883
}

#[derive(Deserialize)]
pub struct MqttConfig {
    #[serde(default = "default_client_id")]
    pub client_id: String,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub auth: Option<MqttAuthConfig>,
    #[serde(default = "Vec::new")]
    pub subscriptions: Vec<String>,
}

#[derive(Deserialize)]
pub struct MqttAuthConfig {
    pub username: String,
    pub password: String,
}

impl TryFrom<Opts> for Config {
    type Error = anyhow::Error;

    fn try_from(opts: Opts) -> Result<Self, Self::Error> {
        Ok(serde_yaml::from_reader(File::open(opts.config)?)?)
    }
}
