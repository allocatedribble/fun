use core::fmt::Debug;

pub trait DenseSlotKey: Copy + Eq + Debug {
    #[must_use]
    fn is_valid(self) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenseSlotMap<K, V> {
    keys: Vec<K>,
    values: Vec<V>,
}

impl<K, V> Default for DenseSlotMap<K, V> {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            values: Vec::new(),
        }
    }
}

impl<K, V> DenseSlotMap<K, V>
where
    K: DenseSlotKey,
{
    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.keys.len() == self.values.len()
    }

    #[must_use]
    pub fn contains_key(&self, key: K) -> bool {
        self.keys.contains(&key)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        debug_assert!(key.is_valid());
        if let Some(index) = self.keys.iter().position(|candidate| *candidate == key) {
            return Some(core::mem::replace(&mut self.values[index], value));
        }
        self.keys.push(key);
        self.values.push(value);
        None
    }

    #[must_use]
    pub fn get(&self, key: K) -> Option<&V> {
        self.keys
            .iter()
            .position(|candidate| *candidate == key)
            .and_then(|index| self.values.get(index))
    }

    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        self.keys
            .iter()
            .position(|candidate| *candidate == key)
            .and_then(|index| self.values.get_mut(index))
    }

    #[must_use]
    pub fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        Some((self.keys.get(index)?, self.values.get(index)?))
    }

    #[must_use]
    pub fn keys(&self) -> &[K] {
        &self.keys
    }

    #[must_use]
    pub fn values(&self) -> &[V] {
        &self.values
    }

    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.keys.iter().copied().zip(self.values.iter())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingBuffer<T> {
    records: Vec<T>,
    capacity: usize,
    start: usize,
}

impl<T> Default for RingBuffer<T> {
    fn default() -> Self {
        Self {
            records: Vec::new(),
            capacity: 0,
            start: 0,
        }
    }
}

impl<T> RingBuffer<T> {
    #[must_use]
    pub const fn with_capacity(capacity: usize) -> Self {
        Self {
            records: Vec::new(),
            capacity,
            start: 0,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn push(&mut self, record: T) {
        if self.capacity == 0 {
            return;
        }
        if self.records.len() < self.capacity {
            self.records.push(record);
            return;
        }
        self.records[self.start] = record;
        self.start = (self.start + 1) % self.capacity;
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        (0..self.records.len()).map(|offset| {
            let index = (self.start + offset) % self.records.len();
            &self.records[index]
        })
    }
}
