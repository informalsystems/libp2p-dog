use fnv::FnvHashMap;
use std::{collections::VecDeque, time::Duration};
use web_time::Instant;

struct ExpiringEntry<K> {
    key: K,
    expiration: Instant,
}

pub(crate) struct DuplicateCache<K, V> {
    /// Size of the cache.
    len: usize,
    /// Map of keys to values in the cache.
    values: FnvHashMap<K, V>,
    /// List of keys in order of expiration.
    list: VecDeque<ExpiringEntry<K>>,
    /// The time values remain in the cache.
    ttl: Duration,
}

impl<K, V> DuplicateCache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    pub(crate) fn new(ttl: Duration) -> Self {
        DuplicateCache {
            len: 0,
            values: FnvHashMap::default(),
            list: VecDeque::new(),
            ttl,
        }
    }

    fn remove_expired_values(&mut self, now: Instant) {
        while let Some(entry) = self.list.pop_front() {
            if entry.expiration > now {
                self.list.push_front(entry);
                break;
            }
            self.len -= 1;
            self.values.remove(&entry.key);
        }
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> bool {
        let now = Instant::now();
        self.remove_expired_values(now);

        if !self.values.contains_key(&key) {
            self.values.insert(key.clone(), value);
            self.len += 1;
            self.list.push_back(ExpiringEntry {
                key,
                expiration: now + self.ttl,
            });
            true
        } else {
            false
        }
    }

    pub(crate) fn contains(&self, key: &K) -> bool {
        self.values.contains_key(key)
    }

    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.values.get(key)
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }
}
