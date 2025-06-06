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

use crate::grpcapi::worker::WorkerGrpcApi;
use crate::service::ApiServices;
use golem_api_grpc::proto;
use golem_api_grpc::proto::golem::worker::v1::worker_service_server::WorkerServiceServer;
use std::net::SocketAddr;
use tonic::codec::CompressionEncoding;
use tonic::transport::{Error, Server};

mod worker;

pub async fn start_grpc_server(addr: SocketAddr, services: ApiServices) -> Result<(), Error> {
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();

    health_reporter
        .set_serving::<WorkerServiceServer<WorkerGrpcApi>>()
        .await;

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build_v1()
        .unwrap();

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
        .serve(addr)
        .await
}
