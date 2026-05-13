use core::marker::PhantomData;

use crate::{FunEntity, FunWorld};

pub trait Component: 'static {}
pub trait Resource: 'static {}
pub trait Bundle: 'static {}
pub trait Message: 'static {}
pub trait SystemSet: Copy + Eq + core::hash::Hash + 'static {}

pub type Entity = FunEntity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedComponents<T> {
    removed: Vec<Entity>,
    marker: PhantomData<T>,
}

impl<T> Default for RemovedComponents<T> {
    fn default() -> Self {
        Self {
            removed: Vec::new(),
            marker: PhantomData,
        }
    }
}

impl<T> RemovedComponents<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, entity: Entity) {
        self.removed.push(entity);
    }

    pub fn clear(&mut self) {
        self.removed.clear();
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.removed.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.removed.len()
    }

    pub fn read(&mut self) -> impl Iterator<Item = Entity> + '_ {
        self.removed.drain(..)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventQueue<T> {
    events: Vec<T>,
}

impl<T> Default for EventQueue<T> {
    fn default() -> Self {
        Self { events: Vec::new() }
    }
}

impl<T> EventQueue<T> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: T) {
        self.events.push(event);
    }

    pub fn write(&mut self, event: T) {
        self.push(event);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.events.drain(..)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.events.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

pub type Messages<T> = EventQueue<T>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Schedule;

impl Schedule {
    pub fn add_systems<T>(&mut self, _systems: T) -> &mut Self {
        self
    }

    pub fn run(&mut self, _world: &mut FunWorld) {}
}

pub trait IntoScheduleConfigs: Sized {
    #[must_use]
    fn chain(self) -> Self {
        self
    }

    #[must_use]
    fn after<T>(self, _set: T) -> Self {
        self
    }

    #[must_use]
    fn in_set<T>(self, _set: T) -> Self {
        self
    }
}

impl<T> IntoScheduleConfigs for T {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowCreated {
    pub window_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowResized {
    pub window_id: u64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowCloseRequested {
    pub window_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InputReceived {
    pub window_id: u64,
    pub input_kind: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceLost {
    pub surface_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceReconfigured {
    pub surface_id: u64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RendererDeviceLost {
    pub adapter_id: u64,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EngineExitRequested {
    pub code: i32,
}
