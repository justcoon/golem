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

mod component;
mod plugin;
mod plugin_installation;

pub use self::component::*;
pub use self::plugin::*;
pub use self::plugin_installation::*;
use sqlx::query_builder::Separated;
use sqlx::{Database, QueryBuilder};
use std::fmt::Display;

pub trait RowMeta<DB: Database> {
    fn add_column_list<Sep: Display>(builder: &mut Separated<DB, Sep>);
    fn add_where_clause<'a>(&'a self, builder: &mut QueryBuilder<'a, DB>);
    fn push_bind<'a, Sep: Display>(&'a self, builder: &mut Separated<'_, 'a, DB, Sep>);
}
