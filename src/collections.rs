use std::collections::{HashMap, VecDeque};
use core::hash::Hash;

pub struct QueueMap<K, V> 
where 
    K: Copy,
    K: PartialEq + Eq,
    K: Hash,
{
    queue: VecDeque<K>,
    set: HashMap<K, V>,
}

impl<K, V> QueueMap<K, V>
where 
    K: Copy,
    K: PartialEq + Eq,
    K: Hash 
{
    pub fn new() -> Self {
        QueueMap {
            queue: VecDeque::new(),
            set: HashMap::new(),
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.set.get(key)
    }

    /// Inserts an item into the QueueSet. Returns `false` if 
    /// the key was already present.
    pub fn push_back(&mut self, key: K, value: V) -> bool {
        if self.set.contains_key(&key) {
            return false;
        } else {
            self.set.insert(key, value);
            self.queue.push_back(key);
            return true;
        }
    }

    /// Pops the oldest item from the QueueSet. 
    pub fn pop_front(&mut self) -> Option<(K, V)> {
        if let Some(last_key) = self.queue.pop_front() {
            self.set.remove_entry(&last_key)
        } else {
            None
        }
    }
}