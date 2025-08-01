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

use crate::durable_host::serialized::SerializableError;
use crate::durable_host::{Durability, DurabilityHost, DurableWorkerCtx, SuspendForSleep};
use crate::workerctx::WorkerCtx;
use chrono::{Duration, Utc};
use golem_common::model::oplog::DurableFunctionType;
use golem_service_base::error::worker_executor::InterruptKind;
use wasmtime::component::Resource;
use wasmtime_wasi::p2::bindings::io::poll::{Host, HostPollable, Pollable};

impl<Ctx: WorkerCtx> HostPollable for DurableWorkerCtx<Ctx> {
    async fn ready(&mut self, self_: Resource<Pollable>) -> anyhow::Result<bool> {
        self.observe_function_call("io::poll:pollable", "ready");
        HostPollable::ready(&mut self.as_wasi_view().0, self_).await
    }

    async fn block(&mut self, self_: Resource<Pollable>) -> anyhow::Result<()> {
        self.observe_function_call("io::poll:pollable", "block");
        let in_ = vec![self_];
        let _ = self.poll(in_).await?;

        Ok(())
    }

    fn drop(&mut self, rep: Resource<Pollable>) -> anyhow::Result<()> {
        self.observe_function_call("io::poll:pollable", "drop");
        HostPollable::drop(&mut self.as_wasi_view().0, rep)
    }
}

impl<Ctx: WorkerCtx> Host for DurableWorkerCtx<Ctx> {
    async fn poll(&mut self, in_: Vec<Resource<Pollable>>) -> anyhow::Result<Vec<u32>> {
        let durability = Durability::<Vec<u32>, SerializableError>::new(
            self,
            "golem io::poll",
            "poll",
            DurableFunctionType::ReadLocal,
        )
        .await?;

        let result = if durability.is_live() {
            let count = in_.len();
            let result = Host::poll(&mut self.as_wasi_view().0, in_).await;
            if is_suspend_for_sleep(&result).is_none() {
                durability.persist(self, count, result).await
            } else {
                result
            }
        } else {
            durability.replay(self).await
        };

        match is_suspend_for_sleep(&result) {
            Some(duration) => {
                self.state.sleep_until(Utc::now() + duration).await?;
                Err(InterruptKind::Suspend.into())
            }
            None => result,
        }
    }
}

fn is_suspend_for_sleep<T>(result: &Result<T, anyhow::Error>) -> Option<Duration> {
    if let Err(err) = result {
        if let Some(SuspendForSleep(duration)) = err.root_cause().downcast_ref::<SuspendForSleep>()
        {
            Some(Duration::from_std(*duration).unwrap())
        } else {
            None
        }
    } else {
        None
    }
}
