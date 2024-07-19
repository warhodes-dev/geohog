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

struct QueueMapIterator<'map, K, V>
where 
    K: Copy,
    K: PartialEq + Eq,
    K: Hash 
{
    map: &'map QueueMap<K, V>,
    index: usize, 
}

impl<'map, K, V> Iterator for QueueMapIterator<'map, K, V>
where 
    K: Copy,
    K: PartialEq + Eq,
    K: Hash 
{
    type Item = &'map V;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.map.queue.len() {
            let key = &self.map.queue[self.index];
            let value = self.map.set.get(key).expect("Desynced set and queue");
            self.index += 1;
            Some(value)
        } else {
            None
        }
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserting_and_popping() {
        
        struct NonCopy(String);

        impl NonCopy {
            fn new(s: &str) -> Self { NonCopy(s.to_owned()) }
        }

        let mut queue = QueueMap::new();

        queue.push_back(10, NonCopy::new("first"));
        queue.push_back(20, NonCopy::new("second"));
        queue.push_back(30, NonCopy::new("third"));
        queue.push_back(30, NonCopy::new("duplicate"));
        queue.push_back(40, NonCopy::new("fourth"));
        queue.push_back(50, NonCopy::new("fifth"));

        assert_eq!(

        );

        queue.get(&30);
        queue.get(&50);
        queue.get(&70);

        queue.pop_front();
        queue.pop_front();

        queue.push_back(60, NonCopy::new("sixth"));
        queue.push_back(70, NonCopy::new("seventh"));
        queue.push_back(50, NonCopy::new("duplicate"));

        queue.pop_front();
        queue.pop_front();
        queue.pop_front();
        queue.pop_front();

        queue.pop_front();
        queue.pop_front();
        queue.pop_front();
        queue.pop_front();
        queue.pop_front();
    }
}
*/













