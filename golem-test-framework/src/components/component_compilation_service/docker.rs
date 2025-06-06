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

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::components::cloud_service::CloudService;
use crate::components::component_compilation_service::ComponentCompilationService;
use crate::components::component_service::ComponentService;
use crate::components::docker::{network, ContainerHandle};
use async_trait::async_trait;
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{Image, ImageExt};
use tracing::{info, Level};

pub struct DockerComponentCompilationService {
    container: ContainerHandle<GolemComponentCompilationServiceImage>,
    public_http_port: u16,
    public_grpc_port: u16,
}

impl DockerComponentCompilationService {
    pub const NAME: &'static str = "golem_component_compilation_service";
    pub const HTTP_PORT: ContainerPort = ContainerPort::Tcp(8083);
    pub const GRPC_PORT: ContainerPort = ContainerPort::Tcp(9094);

    pub async fn new(
        unique_network_id: &str,
        component_service: Arc<dyn ComponentService + Send + Sync + 'static>,
        verbosity: Level,
        cloud_service: Arc<dyn CloudService>,
    ) -> Self {
        info!("Starting golem-component-compilation-service container");

        let env_vars = super::env_vars(
            Self::HTTP_PORT.as_u16(),
            Self::GRPC_PORT.as_u16(),
            component_service,
            &cloud_service,
            verbosity,
        )
        .await;

        let container =
            GolemComponentCompilationServiceImage::new(Self::GRPC_PORT, Self::HTTP_PORT, env_vars)
                .with_network(network(unique_network_id))
                .with_container_name(Self::NAME)
                .start()
                .await
                .expect("Failed to start golem-component-compilation-service container");

        let public_http_port = container
            .get_host_port_ipv4(Self::HTTP_PORT)
            .await
            .expect("Failed to get public HTTP port");

        let public_grpc_port = container
            .get_host_port_ipv4(Self::GRPC_PORT)
            .await
            .expect("Failed to get public gRPC port");

        Self {
            container: ContainerHandle::new(container),
            public_http_port,
            public_grpc_port,
        }
    }
}

#[async_trait]
impl ComponentCompilationService for DockerComponentCompilationService {
    fn private_host(&self) -> String {
        Self::NAME.to_string()
    }

    fn private_http_port(&self) -> u16 {
        Self::HTTP_PORT.as_u16()
    }

    fn private_grpc_port(&self) -> u16 {
        Self::GRPC_PORT.as_u16()
    }

    fn public_host(&self) -> String {
        "localhost".to_string()
    }

    fn public_http_port(&self) -> u16 {
        self.public_http_port
    }

    fn public_grpc_port(&self) -> u16 {
        self.public_grpc_port
    }

    async fn kill(&self) {
        self.container.kill().await
    }
}

#[derive(Debug)]
struct GolemComponentCompilationServiceImage {
    env_vars: HashMap<String, String>,
    expose_ports: [ContainerPort; 2],
}

impl GolemComponentCompilationServiceImage {
    pub fn new(
        grpc_port: ContainerPort,
        http_port: ContainerPort,
        env_vars: HashMap<String, String>,
    ) -> GolemComponentCompilationServiceImage {
        GolemComponentCompilationServiceImage {
            env_vars,
            expose_ports: [grpc_port, http_port],
        }
    }
}

impl Image for GolemComponentCompilationServiceImage {
    fn name(&self) -> &str {
        "golemservices/golem-component-compilation-service"
    }

    fn tag(&self) -> &str {
        "latest"
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("server started")]
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<Item = (impl Into<Cow<'_, str>>, impl Into<Cow<'_, str>>)> {
        self.env_vars.iter()
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &self.expose_ports
    }
}
