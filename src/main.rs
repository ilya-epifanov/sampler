use clap::Parser;
use tokio::join;
use sampler::opts::{Opts, Config};
use rumqttc::{MqttOptions, QoS, Event, Packet, Publish, AsyncClient};
use serde::Deserialize;
use log::{debug, error, info};
// use tts::Tts;
use std::{io::BufReader, convert::TryInto};
use rodio::{Sink, Device, Decoder, OutputStream};
use std::collections::HashMap;
use std::fs::File;
use anyhow::Error;
use tokio::time;
use std::time::Duration;
use std::cell::RefCell;
use cpal::traits::{DeviceTrait, HostTrait};
use itertools::Itertools;

struct Sample {
    sink: Sink,
    _stream: OutputStream,
    config: SamplePlaybackConfig,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
enum PlaybackMode {
    #[serde(rename="one-shot")]
    OneShot,
    #[serde(rename="repeat")]
    Repeat,
    #[serde(rename="stop")]
    Stop,
}

/*
{"filename": "res/heavy-thunder-03.wav"}
*/
#[derive(Deserialize)]
struct SamplePlaybackConfigJson {
    id: Option<String>,
    filename: String,
    volume: Option<f32>,
    restart: Option<bool>,
    mode: Option<PlaybackMode>,
}

#[derive(Debug)]
struct SamplePlaybackConfig {
    id: String,
    canonical_filename: String,
    volume: f32,
    restart: bool,
    mode: PlaybackMode,
}

impl From<SamplePlaybackConfigJson> for SamplePlaybackConfig {
    fn from(config: SamplePlaybackConfigJson) -> Self {
        Self {
            id: config.id.unwrap_or(config.filename.clone()),
            canonical_filename: config.filename,
            volume: config.volume.unwrap_or(1.0),
            restart: config.restart.unwrap_or(false),
            mode: config.mode.unwrap_or(PlaybackMode::OneShot),
        }
    }
}

struct Sampler {
    device: Device,
    samples: HashMap<String, Sample>,
}

impl Sampler {
    pub fn new(device: Device) -> Self {
        Self {
            device,
            samples: Default::default(),
        }
    }

    fn file_source(filename: &str) -> Decoder<BufReader<File>> {
        rodio::Decoder::new(BufReader::new(File::open(filename).unwrap())).unwrap()
    }

    pub fn play(&mut self, config: SamplePlaybackConfig) -> Result<(), Error> {
        let k = config.id.clone();

        info!("playing {:?}", &config);

        if self.samples.contains_key(&k) {
            let sample = self.samples.get_mut(&k).unwrap();
            if config.restart || config.mode != sample.config.mode {
                sample.sink.stop();

                if config.mode != PlaybackMode::Stop {
                    let (_stream, stream_handle) = rodio::OutputStream::try_from_device(&self.device)?;
                    let sink = Sink::try_new(&stream_handle)?;
    
                    sink.set_volume(config.volume);
                    sink.append(Self::file_source(&config.canonical_filename));
                    sink.play();
                    sample.sink = sink;
                    sample._stream = _stream;
                }
                sample.config = config;
            } else {
                sample.sink.set_volume(config.volume);
            }
        } else {
            let (_stream, stream_handle) = rodio::OutputStream::try_from_device(&self.device)?;
            let sink = Sink::try_new(&stream_handle)?;
    
            sink.set_volume(config.volume);
            sink.append(Self::file_source(&config.canonical_filename));
            sink.play();
            let sample = Sample {
                sink,
                _stream,
                config,
            };
            self.samples.insert(k, sample);
        }
        Ok(())
    }

    pub fn gc(&mut self) {
        self.samples.retain(|k, sample| {
            if sample.sink.empty() {
                match sample.config.mode {
                    PlaybackMode::Repeat => {
                        sample.sink.append(Self::file_source(&sample.config.canonical_filename));
                        true
                    },
                    PlaybackMode::OneShot | PlaybackMode::Stop => {
                        info!("removing empty sink for sample {}", k);
                        false
                    },
                }
            } else {
                true
            }
        });
    }
}

async fn mqtt_loop(mqtt_options: MqttOptions, sampler: &RefCell<Sampler>, subscriptions: &[String]) {
    async fn connect_and_loop(mqtt_options: MqttOptions, sampler: &RefCell<Sampler>, subscriptions: &[String]) -> Result<(), anyhow::Error> {
        let (client, mut connection) = AsyncClient::new(mqtt_options, 32);

        for topic in subscriptions {
            info!("subscribing to \"{topic}\"");
            client.subscribe(topic, QoS::AtLeastOnce).await?;
        }

        loop {
            let event = connection.poll().await?;
            if let Event::Incoming(Packet::Publish(Publish { payload, .. })) = event {
                let command: SamplePlaybackConfigJson = serde_json::from_slice(&payload).unwrap();
                if let Err(e) = sampler.borrow_mut().play(command.into()) {
                    error!("can't play sample: {e}");
                };
            }
        }
    }

    loop {
        if let Err(e) = connect_and_loop(mqtt_options.clone(), sampler, subscriptions).await {
            error!("restarting mqtt loop after an error: {e}");
        }
    }
}

async fn sampler_gc_loop(sampler: &RefCell<Sampler>) {
    loop {
        sampler.borrow_mut().gc();
        time::sleep(Duration::from_millis(100)).await;
    }
}


#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    let config: Config = Opts::parse().try_into().unwrap();

    let mut mqtt_options = MqttOptions::new(
        &config.mqtt.client_id, &config.mqtt.host, config.mqtt.port);

    mqtt_options
        .set_clean_session(false)
        .set_inflight(100)
        .set_keep_alive(Duration::from_secs(15));
    if let Some(auth) = &config.mqtt.auth {
        mqtt_options.set_credentials(&auth.username, &auth.password);
    }

    let device: Device = if let Some(device_name) = config.device {
        let devices = cpal::default_host().output_devices()?.collect_vec();

        debug!("devices found:");
        for device in &devices {
            debug!("\t{}", device.name()?);
        }

        devices.into_iter().find(|device: &Device| {
            device.name().ok().filter(|n| n == &device_name).is_some()
        }).unwrap()
    } else {
        cpal::default_host().default_output_device().unwrap()
    };

    // let mut tts = Tts::default()?;
    // tts.speak("test", false)?;

    let sampler = RefCell::new(Sampler::new(device));

    join!(
        mqtt_loop(mqtt_options, &sampler, &config.mqtt.subscriptions),
        sampler_gc_loop(&sampler),
    );

    Ok(())
}
