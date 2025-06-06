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

use crate::gateway_execution::GatewayResolvedWorkerRequest;
use async_trait::async_trait;

use golem_wasm_rpc::protobuf::type_annotated_value::TypeAnnotatedValue;
use std::fmt::Display;

#[async_trait]
pub trait GatewayWorkerRequestExecutor<Namespace> {
    async fn execute(
        &self,
        resolved_worker_request: GatewayResolvedWorkerRequest<Namespace>,
    ) -> Result<WorkerResponse, WorkerRequestExecutorError>;
}

// The result of a worker execution from worker-bridge,
// which is a combination of function metadata and the type-annotated-value representing the actual result
pub struct WorkerResponse {
    pub result: Option<TypeAnnotatedValue>,
}

impl WorkerResponse {
    pub fn new(result: Option<TypeAnnotatedValue>) -> Self {
        WorkerResponse { result }
    }
}

#[derive(Clone, Debug)]
pub struct WorkerRequestExecutorError(String);

impl std::error::Error for WorkerRequestExecutorError {}

impl Display for WorkerRequestExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T: AsRef<str>> From<T> for WorkerRequestExecutorError {
    fn from(err: T) -> Self {
        WorkerRequestExecutorError(err.as_ref().to_string())
    }
}
