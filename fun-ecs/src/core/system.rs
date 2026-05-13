use core::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use fun_scheduler_types::{
    EcsChunkKey, EcsCommandBufferId, EcsComponentId, EcsExternalArtifactKey, EcsLivenessClass,
    EcsResourceId, EcsRunConditionId, EcsSchedulePhase, EcsSpatialDomainKind, EcsSystemClass,
    EcsSystemDescriptor, EcsSystemExecutionContract, EcsSystemId, EcsSystemSetId,
    EcsVirtualResourceKey, EcsWorkKind, EcsWorldRevision, MainThreadRequirement, SystemAccess,
    WorkWaitToken,
};

use crate::{
    FunCommandBufferClass, FunCommandBufferId, FunComponentId, FunEcsComponentKind,
    FunEcsResourceKind, FunEntity, FunExternalSlabId, FunResourceId, FunResourceTableId,
    FunRevision, FunSystemId, FunSystemSetId,
};

pub const FUN_COMMAND_BUFFER_WORLD_STRUCTURE: FunCommandBufferOutput = FunCommandBufferOutput::new(
    FunCommandBufferId::new(1),
    FunCommandBufferClass::WorldStructure,
);
pub const FUN_COMMAND_BUFFER_SPATIAL_REQUESTS: FunCommandBufferOutput = FunCommandBufferOutput::new(
    FunCommandBufferId::new(2),
    FunCommandBufferClass::ResourceTableMutation,
);
pub const FUN_COMMAND_BUFFER_ARTIFACTS: FunCommandBufferOutput = FunCommandBufferOutput::new(
    FunCommandBufferId::new(3),
    FunCommandBufferClass::ArtifactPublication,
);
pub const FUN_COMMAND_BUFFER_DIRTY_PROPAGATION: FunCommandBufferOutput =
    FunCommandBufferOutput::new(
        FunCommandBufferId::new(4),
        FunCommandBufferClass::ResourceTableMutation,
    );
pub const FUN_COMMAND_BUFFER_HANDOFFS: FunCommandBufferOutput = FunCommandBufferOutput::new(
    FunCommandBufferId::new(5),
    FunCommandBufferClass::HandoffPublication,
);

pub type FunSystemClass = EcsSystemClass;
pub type FunSystemExecutionContract = EcsSystemExecutionContract;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunRunConditionId(pub u32);

impl FunRunConditionId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunSystemChunkPolicy {
    #[default]
    WholeWorld = 0,
    ChunkParallel = 1,
    FixedChunk = 2,
    CommandBarrier = 3,
    Diagnostics = 4,
}

impl FunSystemChunkPolicy {
    #[must_use]
    pub const fn work_kind(self) -> EcsWorkKind {
        match self {
            Self::WholeWorld => EcsWorkKind::RunSystem,
            Self::ChunkParallel | Self::FixedChunk => EcsWorkKind::RunSystemChunk,
            Self::CommandBarrier => EcsWorkKind::ApplyCommands,
            Self::Diagnostics => EcsWorkKind::ScheduleDiagnostics,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunExternalWaitSafety {
    #[default]
    Forbidden = 0,
    ExplicitlySafe = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunCommandBufferOutput {
    pub buffer: FunCommandBufferId,
    pub class: FunCommandBufferClass,
}

impl FunCommandBufferOutput {
    #[must_use]
    pub const fn new(buffer: FunCommandBufferId, class: FunCommandBufferClass) -> Self {
        Self { buffer, class }
    }

    #[must_use]
    pub const fn scheduler_id(self) -> EcsCommandBufferId {
        EcsCommandBufferId::new(self.buffer.get() as u64)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FunSystemAccessMode {
    #[default]
    Read = 0,
    Write = 1,
    Output = 2,
    Consume = 3,
    Produce = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunSystemAccessTarget {
    Component(FunComponentId),
    ComponentChunk {
        component: FunComponentId,
        chunk: EcsChunkKey,
    },
    Resource(FunResourceId),
    ResourceTable(FunResourceTableId),
    ResourceTableChunk {
        table: FunResourceTableId,
        chunk: EcsChunkKey,
    },
    WorldStructure,
    VirtualResource(EcsVirtualResourceKey),
    ExternalArtifact(EcsExternalArtifactKey),
    ExternalSlab(FunExternalSlabId),
    CommandBuffer(FunCommandBufferId),
    WaitToken(WorkWaitToken),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunSystemAccessRow {
    pub target: FunSystemAccessTarget,
    pub mode: FunSystemAccessMode,
}

impl FunSystemAccessRow {
    #[must_use]
    pub const fn new(target: FunSystemAccessTarget, mode: FunSystemAccessMode) -> Self {
        Self { target, mode }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FunSystemAccess {
    pub rows: Vec<FunSystemAccessRow>,
}

impl FunSystemAccess {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_row(mut self, row: FunSystemAccessRow) -> Self {
        self.push(row);
        self
    }

    pub fn push(&mut self, row: FunSystemAccessRow) {
        if !self.rows.contains(&row) {
            self.rows.push(row);
            self.rows.sort_unstable();
        }
    }

    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        for row in other.rows {
            self.push(row);
        }
        self
    }

    #[must_use]
    pub fn read_component(component: FunComponentId) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::Component(component),
            FunSystemAccessMode::Read,
        ))
    }

    #[must_use]
    pub fn write_component(component: FunComponentId) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::Component(component),
            FunSystemAccessMode::Write,
        ))
    }

    #[must_use]
    pub fn read_component_kind(kind: FunEcsComponentKind) -> Self {
        Self::read_component(FunComponentId::from_component_kind(kind))
    }

    #[must_use]
    pub fn write_component_kind(kind: FunEcsComponentKind) -> Self {
        Self::write_component(FunComponentId::from_component_kind(kind))
    }

    #[must_use]
    pub fn read_resource(resource: FunResourceId) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::Resource(resource),
            FunSystemAccessMode::Read,
        ))
    }

    #[must_use]
    pub fn write_resource(resource: FunResourceId) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::Resource(resource),
            FunSystemAccessMode::Write,
        ))
    }

    #[must_use]
    pub fn read_resource_kind(kind: FunEcsResourceKind) -> Self {
        Self::read_resource(FunResourceId::from_resource_kind(kind))
    }

    #[must_use]
    pub fn write_resource_kind(kind: FunEcsResourceKind) -> Self {
        Self::write_resource(FunResourceId::from_resource_kind(kind))
    }

    #[must_use]
    pub fn read_table(table: FunResourceTableId) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::ResourceTable(table),
            FunSystemAccessMode::Read,
        ))
    }

    #[must_use]
    pub fn write_table(table: FunResourceTableId) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::ResourceTable(table),
            FunSystemAccessMode::Write,
        ))
    }

    #[must_use]
    pub fn read_table_kind(kind: FunEcsResourceKind) -> Self {
        Self::read_table(FunResourceTableId::from_resource_kind(kind))
    }

    #[must_use]
    pub fn write_table_kind(kind: FunEcsResourceKind) -> Self {
        Self::write_table(FunResourceTableId::from_resource_kind(kind))
    }

    #[must_use]
    pub fn read_table_chunk(table: FunResourceTableId, chunk: EcsChunkKey) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::ResourceTableChunk { table, chunk },
            FunSystemAccessMode::Read,
        ))
    }

    #[must_use]
    pub fn write_table_chunk(table: FunResourceTableId, chunk: EcsChunkKey) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::ResourceTableChunk { table, chunk },
            FunSystemAccessMode::Write,
        ))
    }

    #[must_use]
    pub fn write_world_structure() -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::WorldStructure,
            FunSystemAccessMode::Write,
        ))
    }

    #[must_use]
    pub fn read_virtual_resource(key: EcsVirtualResourceKey) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::VirtualResource(key),
            FunSystemAccessMode::Read,
        ))
    }

    #[must_use]
    pub fn write_virtual_resource(key: EcsVirtualResourceKey) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::VirtualResource(key),
            FunSystemAccessMode::Write,
        ))
    }

    #[must_use]
    pub fn read_external_artifact(key: EcsExternalArtifactKey) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::ExternalArtifact(key),
            FunSystemAccessMode::Read,
        ))
    }

    #[must_use]
    pub fn write_external_artifact(key: EcsExternalArtifactKey) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::ExternalArtifact(key),
            FunSystemAccessMode::Write,
        ))
    }

    #[must_use]
    pub fn read_external_slab(slab: FunExternalSlabId) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::ExternalSlab(slab),
            FunSystemAccessMode::Read,
        ))
    }

    #[must_use]
    pub fn write_external_slab(slab: FunExternalSlabId) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::ExternalSlab(slab),
            FunSystemAccessMode::Write,
        ))
    }

    #[must_use]
    pub fn command_output(output: FunCommandBufferOutput) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::CommandBuffer(output.buffer),
            FunSystemAccessMode::Output,
        ))
    }

    #[must_use]
    pub fn consume_wait_token(token: WorkWaitToken) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::WaitToken(token),
            FunSystemAccessMode::Consume,
        ))
    }

    #[must_use]
    pub fn produce_wait_token(token: WorkWaitToken) -> Self {
        Self::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::WaitToken(token),
            FunSystemAccessMode::Produce,
        ))
    }

    #[must_use]
    pub fn has_world_structure_write(&self) -> bool {
        self.rows.iter().any(|row| {
            row.target == FunSystemAccessTarget::WorldStructure
                && row.mode == FunSystemAccessMode::Write
        })
    }

    #[must_use]
    pub fn touches_external_state(&self) -> bool {
        self.rows.iter().any(|row| {
            matches!(
                row.target,
                FunSystemAccessTarget::ExternalArtifact(_) | FunSystemAccessTarget::ExternalSlab(_)
            )
        })
    }

    #[must_use]
    pub fn requires_main_thread(&self) -> bool {
        false
    }

    #[must_use]
    pub fn has_command_output_for(&self, output: FunCommandBufferOutput) -> bool {
        self.rows.iter().any(|row| {
            row.target == FunSystemAccessTarget::CommandBuffer(output.buffer)
                && row.mode == FunSystemAccessMode::Output
        })
    }

    #[must_use]
    pub fn reads_resource_kind(&self, kind: FunEcsResourceKind) -> bool {
        let resource = FunResourceId::from_resource_kind(kind);
        let table = FunResourceTableId::from_resource_kind(kind);
        self.rows.iter().any(|row| {
            matches!(
                row,
                FunSystemAccessRow {
                    target: FunSystemAccessTarget::Resource(candidate),
                    mode: FunSystemAccessMode::Read,
                } if *candidate == resource
            ) || matches!(
                row,
                FunSystemAccessRow {
                    target: FunSystemAccessTarget::ResourceTable(candidate),
                    mode: FunSystemAccessMode::Read,
                } if *candidate == table
            )
        })
    }

    #[must_use]
    pub fn writes_resource_kind(&self, kind: FunEcsResourceKind) -> bool {
        let resource = FunResourceId::from_resource_kind(kind);
        let table = FunResourceTableId::from_resource_kind(kind);
        self.rows.iter().any(|row| {
            matches!(
                row,
                FunSystemAccessRow {
                    target: FunSystemAccessTarget::Resource(candidate),
                    mode: FunSystemAccessMode::Write,
                } if *candidate == resource
            ) || matches!(
                row,
                FunSystemAccessRow {
                    target: FunSystemAccessTarget::ResourceTable(candidate),
                    mode: FunSystemAccessMode::Write,
                } if *candidate == table
            )
        })
    }

    #[must_use]
    pub fn to_scheduler_access_for<R>(&self) -> SystemAccess<R> {
        let mut access = SystemAccess::new();
        for row in &self.rows {
            access = match (row.target, row.mode) {
                (FunSystemAccessTarget::Component(component), FunSystemAccessMode::Read) => {
                    access.with_read_component(scheduler_component_id(component))
                }
                (FunSystemAccessTarget::Component(component), FunSystemAccessMode::Write) => {
                    access.with_write_component(scheduler_component_id(component))
                }
                (
                    FunSystemAccessTarget::ComponentChunk { component, chunk },
                    FunSystemAccessMode::Read,
                ) => access.with_read_component_chunk(scheduler_component_id(component), chunk),
                (
                    FunSystemAccessTarget::ComponentChunk { component, chunk },
                    FunSystemAccessMode::Write,
                ) => access.with_write_component_chunk(scheduler_component_id(component), chunk),
                (FunSystemAccessTarget::Resource(resource), FunSystemAccessMode::Read) => {
                    access.with_read_resource(scheduler_resource_id(resource))
                }
                (FunSystemAccessTarget::Resource(resource), FunSystemAccessMode::Write) => {
                    access.with_write_resource(scheduler_resource_id(resource))
                }
                (FunSystemAccessTarget::ResourceTable(table), FunSystemAccessMode::Read) => {
                    access.with_read_resource(scheduler_table_resource_id(table))
                }
                (FunSystemAccessTarget::ResourceTable(table), FunSystemAccessMode::Write) => {
                    access.with_write_resource(scheduler_table_resource_id(table))
                }
                (
                    FunSystemAccessTarget::ResourceTableChunk { table, chunk },
                    FunSystemAccessMode::Read,
                ) => access.with_read_resource_chunk(scheduler_table_resource_id(table), chunk),
                (
                    FunSystemAccessTarget::ResourceTableChunk { table, chunk },
                    FunSystemAccessMode::Write,
                ) => access.with_write_resource_chunk(scheduler_table_resource_id(table), chunk),
                (FunSystemAccessTarget::WorldStructure, FunSystemAccessMode::Write) => {
                    access.with_world_structure_write()
                }
                (FunSystemAccessTarget::VirtualResource(key), FunSystemAccessMode::Read) => {
                    access.with_read_virtual_resource(key)
                }
                (FunSystemAccessTarget::VirtualResource(key), FunSystemAccessMode::Write) => {
                    access.with_write_virtual_resource(key)
                }
                (FunSystemAccessTarget::ExternalArtifact(key), FunSystemAccessMode::Read) => {
                    access.with_read_external_artifact(key)
                }
                (FunSystemAccessTarget::ExternalArtifact(key), FunSystemAccessMode::Write) => {
                    access.with_write_external_artifact(key)
                }
                (FunSystemAccessTarget::ExternalSlab(slab), FunSystemAccessMode::Read) => {
                    access.with_read_virtual_resource(external_slab_virtual_resource_key(slab))
                }
                (FunSystemAccessTarget::ExternalSlab(slab), FunSystemAccessMode::Write) => {
                    access.with_write_virtual_resource(external_slab_virtual_resource_key(slab))
                }
                _ => access,
            };
        }
        access
    }

    #[must_use]
    pub fn to_scheduler_access(&self) -> SystemAccess<FunSchedulerEcsRegistry> {
        self.to_scheduler_access_for::<FunSchedulerEcsRegistry>()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunSystemParam {
    Query {
        component: FunComponentId,
        chunk: Option<EcsChunkKey>,
        mutable: bool,
    },
    EntityRef {
        component: FunComponentId,
    },
    EntityMut {
        component: FunComponentId,
    },
    Added {
        component: FunComponentId,
    },
    Changed {
        component: FunComponentId,
    },
    With {
        component: FunComponentId,
    },
    Without {
        component: FunComponentId,
    },
    Resource {
        resource: FunResourceId,
        mutable: bool,
    },
    Table {
        table: FunResourceTableId,
        mutable: bool,
        chunk: Option<EcsChunkKey>,
    },
    ExternalSlab {
        slab: FunExternalSlabId,
        mutable: bool,
    },
    VirtualResource {
        key: EcsVirtualResourceKey,
        mutable: bool,
    },
    ExternalArtifact {
        key: EcsExternalArtifactKey,
        mutable: bool,
    },
    CommandBuffer(FunCommandBufferOutput),
    Event {
        resource: FunResourceId,
        mutable: bool,
    },
    WaitToken {
        token: WorkWaitToken,
        direction: FunWaitTokenDirection,
    },
    NonSendMainThread,
}

impl FunSystemParam {
    #[must_use]
    pub fn access(self) -> FunSystemAccess {
        match self {
            Self::Query {
                component,
                chunk,
                mutable,
            } => match (chunk, mutable) {
                (Some(chunk), false) => FunSystemAccess::new().with_row(FunSystemAccessRow::new(
                    FunSystemAccessTarget::ComponentChunk { component, chunk },
                    FunSystemAccessMode::Read,
                )),
                (Some(chunk), true) => FunSystemAccess::new().with_row(FunSystemAccessRow::new(
                    FunSystemAccessTarget::ComponentChunk { component, chunk },
                    FunSystemAccessMode::Write,
                )),
                (None, false) => FunSystemAccess::read_component(component),
                (None, true) => FunSystemAccess::write_component(component),
            },
            Self::EntityRef { component }
            | Self::Added { component }
            | Self::Changed { component }
            | Self::With { component }
            | Self::Without { component } => FunSystemAccess::read_component(component),
            Self::EntityMut { component } => FunSystemAccess::write_component(component),
            Self::Resource { resource, mutable } => {
                if mutable {
                    FunSystemAccess::write_resource(resource)
                } else {
                    FunSystemAccess::read_resource(resource)
                }
            }
            Self::Table {
                table,
                mutable,
                chunk,
            } => match (chunk, mutable) {
                (Some(chunk), false) => FunSystemAccess::read_table_chunk(table, chunk),
                (Some(chunk), true) => FunSystemAccess::write_table_chunk(table, chunk),
                (None, false) => FunSystemAccess::read_table(table),
                (None, true) => FunSystemAccess::write_table(table),
            },
            Self::ExternalSlab { slab, mutable } => {
                if mutable {
                    FunSystemAccess::write_external_slab(slab)
                } else {
                    FunSystemAccess::read_external_slab(slab)
                }
            }
            Self::VirtualResource { key, mutable } => {
                if mutable {
                    FunSystemAccess::write_virtual_resource(key)
                } else {
                    FunSystemAccess::read_virtual_resource(key)
                }
            }
            Self::ExternalArtifact { key, mutable } => {
                if mutable {
                    FunSystemAccess::write_external_artifact(key)
                } else {
                    FunSystemAccess::read_external_artifact(key)
                }
            }
            Self::CommandBuffer(output) => FunSystemAccess::command_output(output),
            Self::Event { resource, mutable } => {
                if mutable {
                    FunSystemAccess::write_resource(resource)
                } else {
                    FunSystemAccess::read_resource(resource)
                }
            }
            Self::WaitToken { token, direction } => match direction {
                FunWaitTokenDirection::Consume => FunSystemAccess::consume_wait_token(token),
                FunWaitTokenDirection::Produce => FunSystemAccess::produce_wait_token(token),
            },
            Self::NonSendMainThread => FunSystemAccess::new(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunWaitTokenDirection {
    #[default]
    Consume = 0,
    Produce = 1,
}

pub trait FunComponentParam {
    const COMPONENT: FunComponentId;
}

pub trait FunResourceParam {
    const RESOURCE: FunResourceId;
}

pub trait FunTableParam {
    const TABLE: FunResourceTableId;
    const CHUNK: EcsChunkKey = EcsChunkKey::WHOLE_WORLD;
}

pub trait FunExternalSlabParam {
    const SLAB: FunExternalSlabId;
}

pub trait FunVirtualResourceParam {
    const KEY: EcsVirtualResourceKey;
}

pub trait FunExternalArtifactParam {
    const KEY: EcsExternalArtifactKey;
}

pub trait FunEventParam {
    const EVENT_RESOURCE: FunResourceId;
}

pub trait FunQueryFilterAccess {
    #[must_use]
    fn filter_access() -> FunSystemAccess;
}

impl FunQueryFilterAccess for () {
    fn filter_access() -> FunSystemAccess {
        FunSystemAccess::new()
    }
}

impl<A, B> FunQueryFilterAccess for (A, B)
where
    A: FunQueryFilterAccess,
    B: FunQueryFilterAccess,
{
    fn filter_access() -> FunSystemAccess {
        A::filter_access().merge(B::filter_access())
    }
}

impl<A, B, C> FunQueryFilterAccess for (A, B, C)
where
    A: FunQueryFilterAccess,
    B: FunQueryFilterAccess,
    C: FunQueryFilterAccess,
{
    fn filter_access() -> FunSystemAccess {
        A::filter_access()
            .merge(B::filter_access())
            .merge(C::filter_access())
    }
}

pub trait FunSystemParamAccess {
    #[must_use]
    fn system_param() -> FunSystemParam;

    #[must_use]
    fn extra_access() -> FunSystemAccess {
        FunSystemAccess::new()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Query<T, Filter = ()> {
    marker: PhantomData<(T, Filter)>,
}

impl<T, Filter> Query<T, Filter> {
    pub fn iter(&self) -> std::iter::Empty<T> {
        std::iter::empty()
    }

    pub fn iter_mut(&mut self) -> std::iter::Empty<T> {
        std::iter::empty()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        true
    }
}

impl<T, Filter> IntoIterator for &Query<T, Filter> {
    type Item = T;
    type IntoIter = std::iter::Empty<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T, Filter> IntoIterator for &mut Query<T, Filter> {
    type Item = T;
    type IntoIter = std::iter::Empty<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mut<T> {
    value: T,
}

impl<T> Mut<T> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T> Deref for Mut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Mut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityRef<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityMut<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Added<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Changed<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct With<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Without<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Or<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct And<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Res<T> {
    value: T,
}

impl<T> Res<T> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T: Default> Default for Res<T> {
    fn default() -> Self {
        Self {
            value: T::default(),
        }
    }
}

impl<T> Deref for Res<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResMut<T> {
    value: T,
}

impl<T> ResMut<T> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    #[must_use]
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T: Default> Default for ResMut<T> {
    fn default() -> Self {
        Self {
            value: T::default(),
        }
    }
}

impl<T> Deref for ResMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for ResMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableRef<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableMut<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableChunkRef<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableChunkMut<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalSlabRef<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalSlabMut<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VirtualResourceRef<T = ()> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VirtualResourceMut<T = ()> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalArtifactRef<T = ()> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalArtifactMut<T = ()> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Commands;

impl Commands {
    #[must_use]
    pub const fn entity(&mut self, entity: FunEntity) -> EntityCommands {
        EntityCommands { entity }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityCommands {
    entity: FunEntity,
}

impl EntityCommands {
    #[must_use]
    pub const fn id(&self) -> FunEntity {
        self.entity
    }

    pub fn insert<T>(&mut self, _bundle: T) -> &mut Self {
        self
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpatialCommands;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtifactCommands;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandoffCommands;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Events<T> {
    marker: PhantomData<T>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventWriter<T> {
    marker: PhantomData<T>,
}

impl<T> EventWriter<T> {
    pub fn write(&mut self, _event: T) {}

    pub fn send(&mut self, event: T) {
        self.write(event);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventReader<T> {
    marker: PhantomData<T>,
}

impl<T> EventReader<T> {
    pub fn read(&mut self) -> std::iter::Empty<T> {
        std::iter::empty()
    }
}

impl<T, Filter> FunSystemParamAccess for Query<T, Filter>
where
    T: FunComponentParam,
    Filter: FunQueryFilterAccess,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::Query {
            component: T::COMPONENT,
            chunk: None,
            mutable: false,
        }
    }

    fn extra_access() -> FunSystemAccess {
        Filter::filter_access()
    }
}

impl<T> FunSystemParamAccess for EntityRef<T>
where
    T: FunComponentParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::EntityRef {
            component: T::COMPONENT,
        }
    }
}

impl<T> FunSystemParamAccess for EntityMut<T>
where
    T: FunComponentParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::EntityMut {
            component: T::COMPONENT,
        }
    }
}

impl<T> FunQueryFilterAccess for Added<T>
where
    T: FunComponentParam,
{
    fn filter_access() -> FunSystemAccess {
        FunSystemParam::Added {
            component: T::COMPONENT,
        }
        .access()
    }
}

impl<T> FunQueryFilterAccess for Changed<T>
where
    T: FunComponentParam,
{
    fn filter_access() -> FunSystemAccess {
        FunSystemParam::Changed {
            component: T::COMPONENT,
        }
        .access()
    }
}

impl<T> FunQueryFilterAccess for With<T>
where
    T: FunComponentParam,
{
    fn filter_access() -> FunSystemAccess {
        FunSystemParam::With {
            component: T::COMPONENT,
        }
        .access()
    }
}

impl<T> FunQueryFilterAccess for Without<T>
where
    T: FunComponentParam,
{
    fn filter_access() -> FunSystemAccess {
        FunSystemParam::Without {
            component: T::COMPONENT,
        }
        .access()
    }
}

impl<T> FunQueryFilterAccess for Or<T>
where
    T: FunQueryFilterAccess,
{
    fn filter_access() -> FunSystemAccess {
        T::filter_access()
    }
}

impl<T> FunQueryFilterAccess for And<T>
where
    T: FunQueryFilterAccess,
{
    fn filter_access() -> FunSystemAccess {
        T::filter_access()
    }
}

impl<T> FunSystemParamAccess for Res<T>
where
    T: FunResourceParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::Resource {
            resource: T::RESOURCE,
            mutable: false,
        }
    }
}

impl<T> FunSystemParamAccess for ResMut<T>
where
    T: FunResourceParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::Resource {
            resource: T::RESOURCE,
            mutable: true,
        }
    }
}

impl<T> FunSystemParamAccess for TableRef<T>
where
    T: FunTableParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::Table {
            table: T::TABLE,
            mutable: false,
            chunk: None,
        }
    }
}

impl<T> FunSystemParamAccess for TableMut<T>
where
    T: FunTableParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::Table {
            table: T::TABLE,
            mutable: true,
            chunk: None,
        }
    }
}

impl<T> FunSystemParamAccess for TableChunkRef<T>
where
    T: FunTableParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::Table {
            table: T::TABLE,
            mutable: false,
            chunk: Some(T::CHUNK),
        }
    }
}

impl<T> FunSystemParamAccess for TableChunkMut<T>
where
    T: FunTableParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::Table {
            table: T::TABLE,
            mutable: true,
            chunk: Some(T::CHUNK),
        }
    }
}

impl<T> FunSystemParamAccess for ExternalSlabRef<T>
where
    T: FunExternalSlabParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::ExternalSlab {
            slab: T::SLAB,
            mutable: false,
        }
    }
}

impl<T> FunSystemParamAccess for ExternalSlabMut<T>
where
    T: FunExternalSlabParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::ExternalSlab {
            slab: T::SLAB,
            mutable: true,
        }
    }
}

impl<T> FunSystemParamAccess for VirtualResourceRef<T>
where
    T: FunVirtualResourceParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::VirtualResource {
            key: T::KEY,
            mutable: false,
        }
    }
}

impl<T> FunSystemParamAccess for VirtualResourceMut<T>
where
    T: FunVirtualResourceParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::VirtualResource {
            key: T::KEY,
            mutable: true,
        }
    }
}

impl<T> FunSystemParamAccess for ExternalArtifactRef<T>
where
    T: FunExternalArtifactParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::ExternalArtifact {
            key: T::KEY,
            mutable: false,
        }
    }
}

impl<T> FunSystemParamAccess for ExternalArtifactMut<T>
where
    T: FunExternalArtifactParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::ExternalArtifact {
            key: T::KEY,
            mutable: true,
        }
    }
}

impl FunSystemParamAccess for Commands {
    fn system_param() -> FunSystemParam {
        FunSystemParam::CommandBuffer(FUN_COMMAND_BUFFER_WORLD_STRUCTURE)
    }
}

impl FunSystemParamAccess for SpatialCommands {
    fn system_param() -> FunSystemParam {
        FunSystemParam::CommandBuffer(FUN_COMMAND_BUFFER_SPATIAL_REQUESTS)
    }
}

impl FunSystemParamAccess for ArtifactCommands {
    fn system_param() -> FunSystemParam {
        FunSystemParam::CommandBuffer(FUN_COMMAND_BUFFER_ARTIFACTS)
    }
}

impl FunSystemParamAccess for HandoffCommands {
    fn system_param() -> FunSystemParam {
        FunSystemParam::CommandBuffer(FUN_COMMAND_BUFFER_HANDOFFS)
    }
}

impl<T> FunSystemParamAccess for Events<T>
where
    T: FunEventParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::Event {
            resource: T::EVENT_RESOURCE,
            mutable: true,
        }
    }
}

impl<T> FunSystemParamAccess for EventWriter<T>
where
    T: FunEventParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::Event {
            resource: T::EVENT_RESOURCE,
            mutable: true,
        }
    }
}

impl<T> FunSystemParamAccess for EventReader<T>
where
    T: FunEventParam,
{
    fn system_param() -> FunSystemParam {
        FunSystemParam::Event {
            resource: T::EVENT_RESOURCE,
            mutable: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunRunCondition {
    pub id: FunRunConditionId,
    pub label: &'static str,
    pub access: FunSystemAccess,
}

impl FunRunCondition {
    #[must_use]
    pub fn new(id: FunRunConditionId, label: &'static str) -> Self {
        Self {
            id,
            label,
            access: FunSystemAccess::new(),
        }
    }

    #[must_use]
    pub fn with_access(mut self, access: FunSystemAccess) -> Self {
        self.access = self.access.merge(access);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunSystemSet {
    pub id: FunSystemSetId,
    pub label: &'static str,
}

impl FunSystemSet {
    #[must_use]
    pub const fn new(id: FunSystemSetId, label: &'static str) -> Self {
        Self { id, label }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunSystem {
    pub descriptor: FunSystemDescriptor,
}

impl FunSystem {
    #[must_use]
    pub fn new(descriptor: FunSystemDescriptor) -> Self {
        Self { descriptor }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunSystemDescriptor {
    pub id: FunSystemId,
    pub label: &'static str,
    pub class: FunSystemClass,
    pub execution: FunSystemExecutionContract,
    pub phase: EcsSchedulePhase,
    pub set: Option<FunSystemSetId>,
    pub run_condition: Option<FunRunConditionId>,
    pub access: FunSystemAccess,
    pub chunk_policy: FunSystemChunkPolicy,
    pub chunk_key: EcsChunkKey,
    pub world_revision: FunRevision,
    pub liveness_class: EcsLivenessClass,
    pub command_outputs: Vec<FunCommandBufferOutput>,
    pub awaited_tokens: Vec<WorkWaitToken>,
    pub produced_tokens: Vec<WorkWaitToken>,
    pub reads_post_command_state: bool,
    pub exclusive: bool,
    pub non_send: bool,
    pub main_thread_only: bool,
    pub external_wait_safety: FunExternalWaitSafety,
    pub direct_world_structure_write: bool,
}

impl FunSystemDescriptor {
    #[must_use]
    pub fn new(id: FunSystemId, label: &'static str) -> Self {
        Self {
            id,
            label,
            class: EcsSystemClass::WorldQuery,
            execution: EcsSystemExecutionContract::DEFAULT,
            phase: EcsSchedulePhase::Update,
            set: None,
            run_condition: None,
            access: FunSystemAccess::new(),
            chunk_policy: FunSystemChunkPolicy::WholeWorld,
            chunk_key: EcsChunkKey::WHOLE_WORLD,
            world_revision: FunRevision::INITIAL,
            liveness_class: EcsLivenessClass::Normal,
            command_outputs: Vec::new(),
            awaited_tokens: Vec::new(),
            produced_tokens: Vec::new(),
            reads_post_command_state: false,
            exclusive: false,
            non_send: false,
            main_thread_only: false,
            external_wait_safety: FunExternalWaitSafety::Forbidden,
            direct_world_structure_write: false,
        }
    }

    #[must_use]
    pub const fn work_kind(&self) -> EcsWorkKind {
        self.chunk_policy.work_kind()
    }

    #[must_use]
    pub fn with_class(mut self, class: FunSystemClass) -> Self {
        self.class = class;
        self
    }

    #[must_use]
    pub fn with_execution(mut self, execution: FunSystemExecutionContract) -> Self {
        self.execution = execution;
        self
    }

    #[must_use]
    pub fn with_phase(mut self, phase: EcsSchedulePhase) -> Self {
        self.phase = phase;
        self
    }

    #[must_use]
    pub fn with_set(mut self, set: FunSystemSetId) -> Self {
        self.set = Some(set);
        self
    }

    #[must_use]
    pub fn with_run_condition(mut self, condition: FunRunConditionId) -> Self {
        self.run_condition = Some(condition);
        self
    }

    #[must_use]
    pub fn with_access(mut self, access: FunSystemAccess) -> Self {
        self.access = self.access.merge(access);
        self
    }

    #[must_use]
    pub fn with_param<P>(self) -> Self
    where
        P: FunSystemParamAccess,
    {
        self.with_param_descriptor(P::system_param())
            .with_access(P::extra_access())
    }

    #[must_use]
    pub fn with_query_filter<Filter>(mut self) -> Self
    where
        Filter: FunQueryFilterAccess,
    {
        self.access = self.access.merge(Filter::filter_access());
        self
    }

    #[must_use]
    pub fn with_param_descriptor(mut self, param: FunSystemParam) -> Self {
        self.access = self.access.merge(param.access());
        match param {
            FunSystemParam::CommandBuffer(output) => self.push_command_output(output),
            FunSystemParam::WaitToken { token, direction } => match direction {
                FunWaitTokenDirection::Consume => self.push_awaited_token(token),
                FunWaitTokenDirection::Produce => self.push_produced_token(token),
            },
            FunSystemParam::NonSendMainThread => {
                self.non_send = true;
                self.main_thread_only = true;
                self.execution = EcsSystemExecutionContract {
                    main_thread: MainThreadRequirement::MainThreadOnly,
                    ..self.execution
                };
            }
            _ => {}
        }
        self
    }

    #[must_use]
    pub fn with_chunk_policy(mut self, policy: FunSystemChunkPolicy) -> Self {
        self.chunk_policy = policy;
        self
    }

    #[must_use]
    pub fn with_chunk_key(mut self, chunk_key: EcsChunkKey) -> Self {
        self.chunk_key = chunk_key;
        if chunk_key != EcsChunkKey::WHOLE_WORLD
            && self.chunk_policy == FunSystemChunkPolicy::WholeWorld
        {
            self.chunk_policy = FunSystemChunkPolicy::FixedChunk;
        }
        self
    }

    #[must_use]
    pub fn with_world_revision(mut self, revision: FunRevision) -> Self {
        self.world_revision = revision;
        self
    }

    #[must_use]
    pub fn with_liveness_class(mut self, class: EcsLivenessClass) -> Self {
        self.liveness_class = class;
        self
    }

    #[must_use]
    pub fn with_command_output(mut self, output: FunCommandBufferOutput) -> Self {
        self.access = self.access.merge(FunSystemAccess::command_output(output));
        self.push_command_output(output);
        self
    }

    #[must_use]
    pub fn with_awaited_token(mut self, token: WorkWaitToken) -> Self {
        self.access = self
            .access
            .merge(FunSystemAccess::consume_wait_token(token));
        self.push_awaited_token(token);
        self
    }

    #[must_use]
    pub fn with_produced_token(mut self, token: WorkWaitToken) -> Self {
        self.access = self
            .access
            .merge(FunSystemAccess::produce_wait_token(token));
        self.push_produced_token(token);
        self
    }

    #[must_use]
    pub fn reads_post_command_state(mut self) -> Self {
        self.reads_post_command_state = true;
        self
    }

    #[must_use]
    pub fn with_exclusive(mut self) -> Self {
        self.exclusive = true;
        self
    }

    #[must_use]
    pub fn with_non_send(mut self) -> Self {
        self.non_send = true;
        self
    }

    #[must_use]
    pub fn with_main_thread_only(mut self) -> Self {
        self.main_thread_only = true;
        self.execution = EcsSystemExecutionContract {
            main_thread: MainThreadRequirement::MainThreadOnly,
            ..self.execution
        };
        self
    }

    #[must_use]
    pub fn with_external_wait_safety(mut self, safety: FunExternalWaitSafety) -> Self {
        self.external_wait_safety = safety;
        self
    }

    #[must_use]
    pub fn with_direct_world_structure_write(mut self) -> Self {
        self.direct_world_structure_write = true;
        self.access = self.access.merge(FunSystemAccess::write_world_structure());
        self
    }

    pub fn validate(&self) -> Result<(), FunSystemValidationError> {
        if !self.id.is_valid() {
            return Err(FunSystemValidationError::InvalidSystemId);
        }
        if self.label.is_empty() {
            return Err(FunSystemValidationError::EmptyLabel);
        }
        if self.access.has_world_structure_write()
            && self.chunk_policy != FunSystemChunkPolicy::CommandBarrier
        {
            return Err(FunSystemValidationError::DirectWorldStructureMutation);
        }
        for output in &self.command_outputs {
            if !self.access.has_command_output_for(*output) {
                return Err(FunSystemValidationError::CommandBufferOutputNotDeclared);
            }
        }
        if self.liveness_class == EcsLivenessClass::Normal && !self.awaited_tokens.is_empty() {
            return Err(FunSystemValidationError::NormalSystemWaits);
        }
        if self.access.touches_external_state()
            && !self.awaited_tokens.is_empty()
            && self.external_wait_safety != FunExternalWaitSafety::ExplicitlySafe
        {
            return Err(FunSystemValidationError::ExternalStateWaitNotDeclaredSafe);
        }
        if (self.non_send || self.access.requires_main_thread())
            && !self.main_thread_only
            && self.execution.main_thread != MainThreadRequirement::MainThreadOnly
        {
            return Err(FunSystemValidationError::NonSendRequiresMainThread);
        }
        if self.command_outputs.len() > 1 {
            return Err(FunSystemValidationError::MultipleCommandBuffersUnsupported);
        }
        Ok(())
    }

    pub fn to_scheduler_descriptor(
        &self,
    ) -> Result<EcsSystemDescriptor<FunSchedulerEcsRegistry>, FunSystemValidationError> {
        self.to_scheduler_descriptor_for::<FunSchedulerEcsRegistry>()
    }

    pub fn to_scheduler_descriptor_for<R>(
        &self,
    ) -> Result<EcsSystemDescriptor<R>, FunSystemValidationError> {
        self.validate()?;
        let mut descriptor = EcsSystemDescriptor::new(scheduler_system_id(self.id), self.label)
            .with_access(self.access.to_scheduler_access_for::<R>())
            .with_class(self.class)
            .with_execution(self.execution)
            .with_phase(self.phase)
            .with_world_revision(EcsWorldRevision::new(self.world_revision.get()))
            .with_liveness_class(self.liveness_class);

        if let Some(set) = self.set {
            descriptor = descriptor.with_set(EcsSystemSetId::new(set.get()));
        }
        if let Some(condition) = self.run_condition {
            descriptor = descriptor.with_run_condition(EcsRunConditionId::new(condition.get()));
        }
        if let Some(output) = self.command_outputs.first() {
            descriptor = descriptor.with_commands(output.scheduler_id());
        }
        if self.reads_post_command_state {
            descriptor = descriptor.reads_post_command_state();
        }
        if self.exclusive {
            descriptor = descriptor.with_exclusive();
        }
        if self.non_send {
            descriptor = descriptor.with_non_send();
        }
        if self.main_thread_only {
            descriptor = descriptor.with_main_thread_only();
        }
        if self.chunk_key != EcsChunkKey::WHOLE_WORLD {
            descriptor = descriptor.with_chunk_key(self.chunk_key);
        }
        for token in &self.awaited_tokens {
            descriptor = descriptor.with_awaited_token(*token);
        }
        for token in &self.produced_tokens {
            descriptor = descriptor.with_produced_token(*token);
        }
        Ok(descriptor)
    }

    fn push_command_output(&mut self, output: FunCommandBufferOutput) {
        if !self.command_outputs.contains(&output) {
            self.command_outputs.push(output);
            self.command_outputs.sort_unstable();
        }
    }

    fn push_awaited_token(&mut self, token: WorkWaitToken) {
        if !self.awaited_tokens.contains(&token) {
            self.awaited_tokens.push(token);
            self.awaited_tokens.sort_unstable();
        }
    }

    fn push_produced_token(&mut self, token: WorkWaitToken) {
        if !self.produced_tokens.contains(&token) {
            self.produced_tokens.push(token);
            self.produced_tokens.sort_unstable();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunSystemValidationError {
    InvalidSystemId = 0,
    EmptyLabel = 1,
    DirectWorldStructureMutation = 2,
    CommandBufferOutputNotDeclared = 3,
    NormalSystemWaits = 4,
    ExternalStateWaitNotDeclaredSafe = 5,
    NonSendRequiresMainThread = 6,
    MultipleCommandBuffersUnsupported = 7,
}

impl FunSystemValidationError {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::InvalidSystemId => "invalid_system_id",
            Self::EmptyLabel => "empty_label",
            Self::DirectWorldStructureMutation => "direct_world_structure_mutation",
            Self::CommandBufferOutputNotDeclared => "command_buffer_output_not_declared",
            Self::NormalSystemWaits => "normal_system_waits",
            Self::ExternalStateWaitNotDeclaredSafe => "external_state_wait_not_declared_safe",
            Self::NonSendRequiresMainThread => "non_send_requires_main_thread",
            Self::MultipleCommandBuffersUnsupported => "multiple_command_buffers_unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunSchedulerEcsRegistry {}

#[must_use]
pub const fn scheduler_system_id(id: FunSystemId) -> EcsSystemId {
    EcsSystemId::new(id.get())
}

#[must_use]
pub const fn scheduler_component_id(id: FunComponentId) -> EcsComponentId {
    EcsComponentId::new(id.get())
}

#[must_use]
pub const fn scheduler_resource_id(id: FunResourceId) -> EcsResourceId {
    EcsResourceId::new(id.get())
}

#[must_use]
pub const fn scheduler_table_resource_id(id: FunResourceTableId) -> EcsResourceId {
    EcsResourceId::new(id.get())
}

#[must_use]
pub const fn external_slab_virtual_resource_key(slab: FunExternalSlabId) -> EcsVirtualResourceKey {
    EcsVirtualResourceKey::new(
        EcsSpatialDomainKind::Debug,
        0x51_ab,
        0,
        0,
        EcsChunkKey::new(slab.get()),
        0,
    )
}

#[cfg(test)]
mod tests {
    use fun_scheduler_types::{
        EcsAccess, EcsAccessTarget, EcsLivenessClass, EcsSpatialDomainKind, ScheduleDomain,
    };

    use crate::{ScheduleLane, WorkBlockingClass};

    use super::*;

    struct CameraComponent;
    impl FunComponentParam for CameraComponent {
        const COMPONENT: FunComponentId =
            FunComponentId::from_component_kind(FunEcsComponentKind::StreamCamera);
    }

    struct ResidencyTable;
    impl FunTableParam for ResidencyTable {
        const TABLE: FunResourceTableId =
            FunResourceTableId::from_resource_kind(FunEcsResourceKind::PageResidencyTable);
        const CHUNK: EcsChunkKey = EcsChunkKey::new(44);
    }

    struct DiagnosticsEvent;
    impl FunEventParam for DiagnosticsEvent {
        const EVENT_RESOURCE: FunResourceId =
            FunResourceId::from_resource_kind(FunEcsResourceKind::TelemetryEventQueue);
    }

    struct TerrainVirtualResource;
    impl FunVirtualResourceParam for TerrainVirtualResource {
        const KEY: EcsVirtualResourceKey = EcsVirtualResourceKey::new(
            EcsSpatialDomainKind::Terrain,
            0,
            0,
            1,
            EcsChunkKey::new(7),
            3,
        );
    }

    #[test]
    fn param_markers_extract_component_table_event_and_command_access() {
        let descriptor = FunSystemDescriptor::new(FunSystemId::new(1), "extract")
            .with_param::<Query<CameraComponent, With<CameraComponent>>>()
            .with_param::<TableChunkMut<ResidencyTable>>()
            .with_param::<EventWriter<DiagnosticsEvent>>()
            .with_param::<SpatialCommands>();

        assert!(descriptor.access.rows.contains(&FunSystemAccessRow::new(
            FunSystemAccessTarget::Component(CameraComponent::COMPONENT),
            FunSystemAccessMode::Read,
        )));
        assert!(descriptor.access.rows.contains(&FunSystemAccessRow::new(
            FunSystemAccessTarget::ResourceTableChunk {
                table: ResidencyTable::TABLE,
                chunk: ResidencyTable::CHUNK,
            },
            FunSystemAccessMode::Write,
        )));
        assert!(
            descriptor
                .access
                .has_command_output_for(FUN_COMMAND_BUFFER_SPATIAL_REQUESTS)
        );
        assert_eq!(
            descriptor.command_outputs,
            vec![FUN_COMMAND_BUFFER_SPATIAL_REQUESTS]
        );
        assert_eq!(descriptor.validate(), Ok(()));
    }

    #[test]
    fn descriptor_bridges_to_scheduler_access_rows_and_contract() {
        let descriptor = FunSystemDescriptor::new(FunSystemId::new(2), "decode")
            .with_class(EcsSystemClass::Decode)
            .with_execution(EcsSystemExecutionContract {
                domain: ScheduleDomain::Blocking,
                lane: ScheduleLane::Blocking,
                blocking_class: WorkBlockingClass::BlockingPool,
                ..EcsSystemExecutionContract::DEFAULT
            })
            .with_param::<TableChunkRef<ResidencyTable>>()
            .with_param::<VirtualResourceRef<TerrainVirtualResource>>()
            .with_chunk_policy(FunSystemChunkPolicy::ChunkParallel)
            .with_chunk_key(EcsChunkKey::new(44));

        let scheduler = descriptor
            .to_scheduler_descriptor()
            .expect("scheduler descriptor");

        assert_eq!(descriptor.work_kind(), EcsWorkKind::RunSystemChunk);
        assert_eq!(scheduler.class, EcsSystemClass::Decode);
        assert_eq!(scheduler.execution.domain, ScheduleDomain::Blocking);
        assert_eq!(scheduler.chunk_key, EcsChunkKey::new(44));
        assert!(scheduler.access.accesses.iter().any(|row| matches!(
            row,
            EcsAccess {
                target: EcsAccessTarget::ResourceChunk { .. },
                ..
            }
        )));
        assert!(scheduler.access.accesses.iter().any(|row| matches!(
            row,
            EcsAccess {
                target: EcsAccessTarget::VirtualResource { .. },
                ..
            }
        )));
    }

    #[test]
    fn hard_rules_reject_direct_structure_waits_and_unsafe_external_waits() {
        let direct = FunSystemDescriptor::new(FunSystemId::new(3), "direct")
            .with_direct_world_structure_write();
        assert_eq!(
            direct.validate(),
            Err(FunSystemValidationError::DirectWorldStructureMutation)
        );

        let token = WorkWaitToken::new(77);
        let normal_wait =
            FunSystemDescriptor::new(FunSystemId::new(4), "wait").with_awaited_token(token);
        assert_eq!(
            normal_wait.validate(),
            Err(FunSystemValidationError::NormalSystemWaits)
        );

        let external_wait = FunSystemDescriptor::new(FunSystemId::new(5), "external_wait")
            .with_param::<VirtualResourceRef<TerrainVirtualResource>>()
            .with_access(FunSystemAccess::read_external_slab(FunExternalSlabId::new(
                9,
            )))
            .with_liveness_class(EcsLivenessClass::SchedulerWait)
            .with_awaited_token(token);
        assert_eq!(
            external_wait.validate(),
            Err(FunSystemValidationError::ExternalStateWaitNotDeclaredSafe)
        );

        let safe_external_wait =
            external_wait.with_external_wait_safety(FunExternalWaitSafety::ExplicitlySafe);
        assert_eq!(safe_external_wait.validate(), Ok(()));
    }

    #[test]
    fn non_send_params_pin_main_thread() {
        let descriptor = FunSystemDescriptor::new(FunSystemId::new(6), "non_send")
            .with_param_descriptor(FunSystemParam::NonSendMainThread);

        assert!(descriptor.non_send);
        assert!(descriptor.main_thread_only);
        assert_eq!(
            descriptor.execution.main_thread,
            MainThreadRequirement::MainThreadOnly
        );
        assert_eq!(descriptor.validate(), Ok(()));
    }
}
