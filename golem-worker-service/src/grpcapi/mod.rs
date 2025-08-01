// Copyright 2024-2025 Golem Cloud
//
// Licensed under the Golem Source License v1.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://license.golem.cloud/LICENSE
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod error;
mod worker;

use crate::grpcapi::worker::WorkerGrpcApi;
use crate::service::Services;
use futures::TryFutureExt;
use golem_api_grpc::proto;
use golem_api_grpc::proto::golem::common::{ErrorBody, ErrorsBody};
use golem_api_grpc::proto::golem::worker::v1::worker_service_server::WorkerServiceServer;
use golem_api_grpc::proto::golem::worker::v1::{
    worker_error, worker_execution_error, WorkerError, WorkerExecutionError,
};
use golem_common::model::{ComponentFilePath, TargetWorkerId, WorkerId};
use golem_wasm_rpc::json::OptionallyValueAndTypeJson;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::codec::CompressionEncoding;
use tonic::transport::Server;
use tonic::Status;
use tracing::Instrument;

pub async fn start_grpc_server(
    addr: SocketAddr,
    services: Services,
    join_set: &mut JoinSet<Result<(), anyhow::Error>>,
) -> anyhow::Result<u16> {
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();

    let listener = TcpListener::bind(addr).await?;
    let port = listener.local_addr()?.port();

    health_reporter
        .set_serving::<WorkerServiceServer<WorkerGrpcApi>>()
        .await;

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build_v1()
        .unwrap();

    join_set.spawn(
        async move {
            Server::builder()
                .add_service(reflection_service)
                .add_service(health_service)
                .add_service(
                    WorkerServiceServer::new(WorkerGrpcApi::new(
                        services.component_service.clone(),
                        services.worker_service.clone(),
                        services.worker_auth_service.clone(),
                    ))
                    .send_compressed(CompressionEncoding::Gzip)
                    .accept_compressed(CompressionEncoding::Gzip),
                )
                .serve_with_incoming(TcpListenerStream::new(listener))
                .map_err(anyhow::Error::from)
                .await
        }
        .in_current_span(),
    );

    Ok(port)
}

pub fn validated_worker_id(
    component_id: golem_common::model::ComponentId,
    worker_name: String,
) -> Result<WorkerId, WorkerError> {
    WorkerId::validate_worker_name(&worker_name)
        .map_err(|error| bad_request_error(format!("Invalid worker name: {error}")))?;
    Ok(WorkerId {
        component_id,
        worker_name,
    })
}

pub fn validated_target_worker_id(
    component_id: golem_common::model::ComponentId,
    worker_name: Option<String>,
) -> Result<TargetWorkerId, WorkerError> {
    if let Some(worker_name) = &worker_name {
        WorkerId::validate_worker_name(worker_name)
            .map_err(|error| bad_request_error(format!("Invalid worker name: {error}")))?;
    }
    Ok(TargetWorkerId {
        component_id,
        worker_name,
    })
}

pub fn validate_protobuf_worker_id(
    worker_id: Option<golem_api_grpc::proto::golem::worker::WorkerId>,
) -> Result<WorkerId, WorkerError> {
    let worker_id = worker_id.ok_or_else(|| bad_request_error("Missing worker id"))?;
    let worker_id: WorkerId = worker_id
        .try_into()
        .map_err(|e| bad_request_error(format!("Invalid worker id: {e}")))?;
    validated_worker_id(worker_id.component_id, worker_id.worker_name)
}

pub fn validate_protobuf_target_worker_id(
    worker_id: Option<golem_api_grpc::proto::golem::worker::TargetWorkerId>,
) -> Result<TargetWorkerId, WorkerError> {
    let worker_id = worker_id.ok_or_else(|| bad_request_error("Missing worker id"))?;
    let worker_id: TargetWorkerId = worker_id
        .try_into()
        .map_err(|e| bad_request_error(format!("Invalid target worker id: {e}")))?;
    validated_target_worker_id(worker_id.component_id, worker_id.worker_name)
}

pub fn validate_protobuf_plugin_installation_id(
    plugin_installation_id: Option<golem_api_grpc::proto::golem::common::PluginInstallationId>,
) -> Result<golem_common::model::PluginInstallationId, WorkerError> {
    plugin_installation_id
        .ok_or_else(|| bad_request_error("Missing plugin installation id"))?
        .try_into()
        .map_err(|e| bad_request_error(format!("Invalid plugin installation id: {e}")))
}

pub fn validate_component_file_path(file_path: String) -> Result<ComponentFilePath, WorkerError> {
    ComponentFilePath::from_abs_str(&file_path).map_err(|_| bad_request_error("Invalid file path"))
}

pub fn bad_request_error<T>(error: T) -> WorkerError
where
    T: Into<String>,
{
    WorkerError {
        error: Some(worker_error::Error::BadRequest(ErrorsBody {
            errors: vec![error.into()],
        })),
    }
}

pub fn bad_request_errors(errors: Vec<String>) -> WorkerError {
    WorkerError {
        error: Some(worker_error::Error::BadRequest(ErrorsBody { errors })),
    }
}

pub fn error_to_status(error: WorkerError) -> Status {
    match error.error {
        Some(worker_error::Error::BadRequest(ErrorsBody { errors })) => {
            Status::invalid_argument(format!("Bad Request: {errors:?}"))
        }
        Some(worker_error::Error::Unauthorized(ErrorBody { error })) => {
            Status::unauthenticated(error)
        }
        Some(worker_error::Error::LimitExceeded(ErrorBody { error })) => {
            Status::resource_exhausted(error)
        }
        Some(worker_error::Error::NotFound(ErrorBody { error })) => Status::not_found(error),
        Some(worker_error::Error::AlreadyExists(ErrorBody { error })) => {
            Status::already_exists(error)
        }
        Some(worker_error::Error::InternalError(WorkerExecutionError { error: None })) => {
            Status::unknown("Unknown error")
        }

        Some(worker_error::Error::InternalError(WorkerExecutionError {
            error: Some(worker_execution_error),
        })) => {
            let message = match worker_execution_error {
                worker_execution_error::Error::InvalidRequest(err) => {
                    format!("Invalid Request: {}", err.details)
                }
                worker_execution_error::Error::WorkerAlreadyExists(err) => {
                    format!("Worker Already Exists: Worker ID = {:?}", err.worker_id)
                }
                worker_execution_error::Error::WorkerCreationFailed(err) => format!(
                    "Worker Creation Failed: Worker ID = {:?}, Details: {}",
                    err.worker_id, err.details
                ),
                worker_execution_error::Error::FailedToResumeWorker(err) => {
                    format!("Failed To Resume Worker: Worker ID = {:?}", err.worker_id)
                }
                worker_execution_error::Error::ComponentDownloadFailed(err) => format!(
                    "Component Download Failed: Component ID = {:?}, Version: {}, Reason: {}",
                    err.component_id, err.component_version, err.reason
                ),
                worker_execution_error::Error::ComponentParseFailed(err) => format!(
                    "Component Parsing Failed: Component ID = {:?}, Version: {}, Reason: {}",
                    err.component_id, err.component_version, err.reason
                ),
                worker_execution_error::Error::GetLatestVersionOfComponentFailed(err) => format!(
                    "Get Latest Version Of Component Failed: Component ID = {:?}, Reason: {}",
                    err.component_id, err.reason
                ),
                worker_execution_error::Error::PromiseNotFound(err) => {
                    format!("Promise Not Found: Promise ID = {:?}", err.promise_id)
                }
                worker_execution_error::Error::PromiseDropped(err) => {
                    format!("Promise Dropped: Promise ID = {:?}", err.promise_id)
                }
                worker_execution_error::Error::PromiseAlreadyCompleted(err) => format!(
                    "Promise Already Completed: Promise ID = {:?}",
                    err.promise_id
                ),
                worker_execution_error::Error::Interrupted(err) => format!(
                    "Interrupted: Recover Immediately = {}",
                    err.recover_immediately
                ),
                worker_execution_error::Error::ParamTypeMismatch(_) => {
                    "Parameter Type Mismatch".to_string()
                }
                worker_execution_error::Error::NoValueInMessage(_) => {
                    "No Value In Message".to_string()
                }
                worker_execution_error::Error::ValueMismatch(err) => {
                    format!("Value Mismatch: {}", err.details)
                }
                worker_execution_error::Error::UnexpectedOplogEntry(err) => format!(
                    "Unexpected Oplog Entry: Expected = {}, Got = {}",
                    err.expected, err.got
                ),
                worker_execution_error::Error::RuntimeError(err) => {
                    format!("Runtime Error: {}", err.details)
                }
                worker_execution_error::Error::InvalidShardId(err) => format!(
                    "Invalid Shard ID: Shard ID = {:?}, Shard IDs: {:?}",
                    err.shard_id, err.shard_ids
                ),
                worker_execution_error::Error::PreviousInvocationFailed(_) => {
                    "Previous Invocation Failed".to_string()
                }
                worker_execution_error::Error::PreviousInvocationExited(_) => {
                    "Previous Invocation Exited".to_string()
                }
                worker_execution_error::Error::Unknown(err) => {
                    format!("Unknown Error: {}", err.details)
                }
                worker_execution_error::Error::InvalidAccount(_) => "Invalid Account".to_string(),
                worker_execution_error::Error::WorkerNotFound(err) => {
                    format!("Worker Not Found: Worker ID = {:?}", err.worker_id)
                }
                worker_execution_error::Error::ShardingNotReady(_) => {
                    "Sharding Not Ready".to_string()
                }
                worker_execution_error::Error::InitialComponentFileDownloadFailed(_) => {
                    "Initial File Download Failed".to_string()
                }
                worker_execution_error::Error::FileSystemError(_) => {
                    "Failed accessing worker filesystem".to_string()
                }
                worker_execution_error::Error::InvocationFailed(_) => {
                    "Invocation Failed".to_string()
                }
            };
            Status::internal(message)
        }
        None => Status::unknown("Unknown error"),
    }
}

pub fn parse_json_invoke_parameters(
    parameters: &[String],
) -> Result<Vec<OptionallyValueAndTypeJson>, WorkerError> {
    let optionally_typed_parameters: Vec<OptionallyValueAndTypeJson> = parameters
        .iter()
        .map(|param| serde_json::from_str(param))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| bad_request_error(format!("Failed to parse JSON parameters: {err:?}")))?;

    Ok(optionally_typed_parameters)
}
