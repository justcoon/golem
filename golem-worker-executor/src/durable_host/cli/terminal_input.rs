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

use async_trait::async_trait;
use wasmtime::component::Resource;

use crate::durable_host::{DurabilityHost, DurableWorkerCtx};
use crate::workerctx::WorkerCtx;
use wasmtime_wasi::p2::bindings::cli::terminal_input::{Host, HostTerminalInput, TerminalInput};

#[async_trait]
impl<Ctx: WorkerCtx> HostTerminalInput for DurableWorkerCtx<Ctx> {
    fn drop(&mut self, rep: Resource<TerminalInput>) -> anyhow::Result<()> {
        self.observe_function_call("cli::terminal_input::terminal_input", "drop");
        HostTerminalInput::drop(&mut self.as_wasi_view(), rep)
    }
}

#[async_trait]
impl<Ctx: WorkerCtx> Host for DurableWorkerCtx<Ctx> {}
