use std::{
    collections::HashMap,
    num::NonZeroUsize,
    sync::RwLock,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct Backlog<K, V> {
    incomplete_table: RwLock<HashMap<K, EphemeralVec<V>>>,
    table_max: NonZeroUsize,
}
impl<K, V> Backlog<K, V> {
    pub fn new(table_max: NonZeroUsize) -> Self {
        Self {
            incomplete_table: RwLock::new(HashMap::new()),
            table_max,
        }
    }
    pub fn clean(&self, timeout: Duration) {
        self.incomplete_table
            .write()
            .unwrap()
            .retain(|_, v: &mut EphemeralVec<V>| !v.timed_out(timeout));
    }
}
impl<K, V> Backlog<K, V>
where
    K: Clone + Eq + core::hash::Hash,
{
    pub fn handle(&self, key: K, value: V, size: NonZeroUsize) -> Option<Vec<V>> {
        let mut incomplete_table = self.incomplete_table.write().unwrap();
        match incomplete_table.remove(&key) {
            Some(incomplete_list) => {
                let res = incomplete_list.push(value);
                match res {
                    PushResult::Incomplete(incomplete_list) => {
                        incomplete_table.insert(key, incomplete_list);
                        None
                    }
                    PushResult::Complete(list) => Some(list),
                }
            }
            None => {
                if self.table_max.get() <= incomplete_table.len() {
                    return None;
                }
                let incomplete_list = EphemeralVec::new(size);
                incomplete_table.insert(key.clone(), incomplete_list);
                drop(incomplete_table);
                self.handle(key, value, size)
            }
        }
    }
}

#[derive(Debug)]
struct EphemeralVec<T> {
    list: Vec<T>,
    size: NonZeroUsize,
    last_update: Instant,
}
impl<T> EphemeralVec<T> {
    pub fn new(size: NonZeroUsize) -> Self {
        Self {
            list: vec![],
            size,
            last_update: Instant::now(),
        }
    }
    pub fn push(mut self, value: T) -> PushResult<T> {
        self.list.push(value);
        if self.list.len() == self.size.get() {
            return PushResult::Complete(self.list);
        }
        self.last_update = Instant::now();
        PushResult::Incomplete(self)
    }
    pub fn timed_out(&self, timeout: Duration) -> bool {
        self.last_update.elapsed() > timeout
    }
}
#[derive(Debug)]
enum PushResult<T> {
    Incomplete(EphemeralVec<T>),
    Complete(Vec<T>),
}
