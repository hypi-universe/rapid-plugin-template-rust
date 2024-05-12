use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Mutex;

use bytes::Bytes;
use http::StatusCode;
use lazy_static::lazy_static;
use log::{debug, info};
use snowflake::SnowflakeIdGenerator;
use thiserror::Error;
use tokio::fs;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};
use tonic::transport::Server;
use tonic::{async_trait, Request, Response, Status, Streaming};

use rapid_utils::plugin::input_sequence::Value;
use rapid_utils::plugin::plugin_server::{Plugin, PluginServer};
use rapid_utils::plugin::{ input_sequence, output_sequence, Field, InputSequence, OutputMessage, OutputSequence, Pair, Payload, PayloadAction, PayloadEvent, PluginError, StepEnvironment, DataValue};

lazy_static! {
    static ref SNOWFLAKE: Mutex<SnowflakeIdGenerator> = Mutex::new(SnowflakeIdGenerator::new(1, 1));
}

pub fn next_id() -> i64 {
    return SNOWFLAKE
        .lock()
        .unwrap_or_else(|v| v.into_inner())
        .real_time_generate();
}

#[derive(Debug, Error)]
pub enum InternalPluginErr {
    #[error("IO Error. {0}")]
    Io(std::io::Error),
}

pub type Result<T> = std::result::Result<T, InternalPluginErr>;

struct MyPlugin;

#[async_trait]
impl Plugin for MyPlugin {
    type sequenceStream =
        Pin<Box<dyn Stream<Item = std::result::Result<OutputSequence, Status>> + Send + 'static>>;

    async fn sequence(
        &self,
        request: Request<Streaming<InputSequence>>,
    ) -> std::result::Result<Response<Self::sequenceStream>, Status> {
        let mut in_stream = request.into_inner();
        let (tx, rx): (
            Sender<std::result::Result<_, Status>>,
            Receiver<std::result::Result<_, Status>>,
        ) = mpsc::channel(128);
        // this spawn here is required if you want to handle connection error.
        // If we just map `in_stream` and write it back as `out_stream` the `out_stream`
        // will be dropped when connection error occurs and error will never be propagated
        // to mapped version of `in_stream`.
        tokio::spawn(async move {
            debug!("New connection, waiting for user queries");
            let (input_tx, input_rx) = mpsc::channel(1);
            let mut input_stream = Some(ReceiverStream::new(input_rx));
            while let Some(seq) = in_stream.next().await {
                match seq {
                    Ok(seq) => {
                        if let Some(seq) = seq.value {
                            match seq {
                                Value::Step(step_val) => {}
                                Value::InputPayload(payload) => {
                                    let data: Bytes = payload.data.into();
                                    match input_tx.send(data).await {
                                        Ok(_) => if !payload.is_end {},
                                        Err(e) => {
                                            info!(
                                                "Unable to process chunk of request payload. {}",
                                                e
                                            );
                                        }
                                    }
                                }
                                Value::OutputPayloadAction(out_action) => {
                                    match out_action.action() {
                                        PayloadAction::Send => {
                                            // if let Some(file) = &mut get_file {
                                            //     let lim = out_action.limit as usize;
                                            //     let mut buf = Vec::with_capacity(lim);
                                            //     match file.read(&mut buf[..lim]) {
                                            //         Ok(len) => {
                                            //             Self::send_evt_to_rapid_server(
                                            //                 &tx,
                                            //                 output_sequence::Value::OutputPayload(
                                            //                     buf[..len].to_vec(),
                                            //                 ),
                                            //             )
                                            //             .await;
                                            //         }
                                            //         Err(e) => {
                                            //             info!("Unable to read chunk of file for a GET req. {}",e);
                                            //             break;
                                            //         }
                                            //     }
                                            // }
                                        }
                                        PayloadAction::Stop => {}
                                    }
                                }
                                Value::PermissionResponse(_) => {}
                                Value::End(_) => {
                                    info!("RAPID server told me to end processing");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        info!("Input status err. {}", e)
                    }
                }
            }
            info!("\tclient disconnected");
        });

        let out_stream = ReceiverStream::new(rx);

        Ok(Response::new(Box::pin(out_stream) as Self::sequenceStream))
    }
}

impl MyPlugin {
    async fn send_evt_to_rapid_server(
        tx: &Sender<std::result::Result<OutputSequence, Status>>,
        seq: output_sequence::Value,
    ) {
        tx.send(Ok(OutputSequence { value: Some(seq) }))
            .await
            .unwrap()
    }

    async fn send_msg_to_rapid_srv(
        tx: &Sender<std::result::Result<OutputSequence, Status>>,
        status: i32,
        headers: Vec<Pair>,
        data: Vec<DataValue>,
    ) {
        Self::send_evt_to_rapid_server(
            tx,
            output_sequence::Value::Output(OutputMessage {
                status: Some(status),
                headers,
                data,
            }),
        )
        .await;
    }

    async fn send_http_err(
        tx: &Sender<std::result::Result<OutputSequence, Status>>,
        code: String,
        status: StatusCode,
        reason: String,
    ) {
        Self::send_evt_to_rapid_server(
            &tx,
            output_sequence::Value::Error(PluginError {
                status: status.as_u16() as i32,
                code,
                message: reason,
                context: vec![],
            }),
        )
        .await;
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 16)]
async fn main() {
    let _ = simple_logger::init_with_level(log::Level::Info);
    println!("Listening on 2020");
    start_server().await;
}

async fn start_server() {
    Server::builder()
        .accept_http1(true)
        .add_service(PluginServer::new(MyPlugin))
        .serve(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::from_str("0.0.0.0").unwrap()),
            2020, //not configurable, RAPID will always use 2020
        ))
        .await
        .unwrap();
}

#[cfg(test)]
mod test {}
