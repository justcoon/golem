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

use std::fmt::Display;

use crate::config::StaticComponentServiceConfig;
use golem_common::model::{ComponentId, ProjectId};
use tokio::sync::mpsc;
use wasmtime::component::Component;

#[derive(Debug, Clone)]
pub struct ComponentWithVersion {
    pub id: ComponentId,
    pub version: u64,
}

impl Display for ComponentWithVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.id, self.version)
    }
}

#[derive(Debug)]
pub struct CompilationRequest {
    pub component: ComponentWithVersion,
    pub project_id: ProjectId,
    pub sender: Option<StaticComponentServiceConfig>,
}

pub struct CompiledComponent {
    pub component_and_version: ComponentWithVersion,
    pub project_id: ProjectId,
    pub component: Component,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum CompilationError {
    #[error("Component not found: {0}")]
    ComponentNotFound(ComponentWithVersion),
    #[error("Failed to compile component: {0}")]
    CompileFailure(String),
    #[error("Failed to download component: {0}")]
    ComponentDownloadFailed(String),
    #[error("Failed to upload component: {0}")]
    ComponentUploadFailed(String),
    #[error("Unexpected error: {0}")]
    Unexpected(String),
}

impl<T> From<mpsc::error::SendError<T>> for CompilationError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        CompilationError::Unexpected("Failed to send compilation request".to_string())
    }
}
