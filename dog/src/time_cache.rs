use std::{collections::VecDeque, time::Duration};

use fnv::FnvHashSet;
use web_time::Instant;

struct ExpiringValues<Element> {
    value: Element,
    expiration: Instant,
}

pub(crate) struct DuplicateCache<T> {
    /// Size of the cache.
    len: usize,
    /// Set of values in the cache.
    values: FnvHashSet<T>,
    /// List of values in order of expiration.
    list: VecDeque<ExpiringValues<T>>,
    /// The time values remain in the cache.
    ttl: Duration,
}

impl<T> DuplicateCache<T>
where
    T: Eq + std::hash::Hash + Clone,
{
    pub(crate) fn new(ttl: Duration) -> Self {
        DuplicateCache {
            len: 0,
            values: FnvHashSet::default(),
            list: VecDeque::new(),
            ttl,
        }
    }

    fn remove_expired_values(&mut self, now: Instant) {
        while let Some(element) = self.list.pop_front() {
            if element.expiration > now {
                self.list.push_front(element);
                break;
            }
            self.len -= 1;
            self.values.remove(&element.value);
        }
    }

    pub(crate) fn insert(&mut self, value: T) -> bool {
        let now = Instant::now();
        self.remove_expired_values(now);
        if self.values.insert(value.clone()) {
            self.len += 1;
            self.list.push_back(ExpiringValues {
                value,
                expiration: now + self.ttl,
            });
            true
        } else {
            false
        }
    }

    pub(crate) fn contains(&self, value: &T) -> bool {
        self.values.contains(value)
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }
}
