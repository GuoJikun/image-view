//! 通用 LRU 缓存。
//!
//! 提供两种策略：
//! - [`LruCache`]：按条目数淘汰，适合缩略图纹理句柄。
//! - [`BudgetCache`]：按条目数 + 成本（如像素数）淘汰，可保护当前显示的条目。
//!
//! 本 crate 不持有像素或 GUI 资源，只做键值容器。

use std::collections::VecDeque;

/// 最近使用顺序的键值缓存；命中时移到队尾，淘汰从队首开始。
#[derive(Debug)]
pub struct LruCache<K, V> {
    entries: VecDeque<(K, V)>,
    max_entries: usize,
}

impl<K: PartialEq, V: Clone> LruCache<K, V> {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
        }
    }

    /// 取出一个值并刷新其最近使用顺序。
    pub fn get(&mut self, key: &K) -> Option<V> {
        let index = self.entries.iter().position(|(entry, _)| entry == key)?;
        let (key, value) = self.entries.remove(index)?;
        self.entries.push_back((key, value.clone()));
        Some(value)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.entries.iter().any(|(entry, _)| entry == key)
    }

    /// 插入或刷新条目；超过条目上限时淘汰最旧的。
    pub fn insert(&mut self, key: K, value: V) {
        self.insert_with_limit(key, value, self.max_entries);
    }

    /// 以指定上限插入；用于测试或临时收紧缓存预算。
    pub fn insert_with_limit(&mut self, key: K, value: V, limit: usize) {
        self.entries.retain(|(entry, _)| entry != &key);
        self.entries.push_back((key, value));
        while self.entries.len() > limit {
            self.entries.pop_front();
        }
    }

    pub fn remove(&mut self, key: &K) {
        self.entries.retain(|(entry, _)| entry != key);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<K: PartialEq, V: Clone> Default for LruCache<K, V> {
    fn default() -> Self {
        Self::new(0)
    }
}

/// 条目数 + 成本双重上限的 LRU 缓存。
///
/// `protected` 用于当前正在显示的条目：即使它最旧也跳过淘汰，避免界面闪烁。
#[derive(Debug)]
pub struct BudgetCache<K, V> {
    entries: VecDeque<(K, V)>,
    max_entries: usize,
    max_cost: usize,
    cost: fn(&V) -> usize,
}

impl<K: PartialEq, V: Clone> BudgetCache<K, V> {
    pub fn new(max_entries: usize, max_cost: usize, cost: fn(&V) -> usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries,
            max_cost,
            cost,
        }
    }

    /// 取出一个值并刷新其最近使用顺序。
    pub fn get(&mut self, key: &K) -> Option<V> {
        let index = self.entries.iter().position(|(entry, _)| entry == key)?;
        let (key, value) = self.entries.remove(index)?;
        self.entries.push_back((key, value.clone()));
        Some(value)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.entries.iter().any(|(entry, _)| entry == key)
    }

    /// 插入或刷新条目；`protected` 指定的键不会被本次淘汰移除。
    pub fn insert(&mut self, key: K, value: V, protected: Option<&K>) {
        self.entries.retain(|(entry, _)| entry != &key);
        self.entries.push_back((key, value));

        let mut pixels: usize = self
            .entries
            .iter()
            .map(|(_, value)| (self.cost)(value))
            .sum();
        let mut index = 0;
        while (self.entries.len() > self.max_entries || pixels > self.max_cost)
            && index < self.entries.len()
        {
            // 当前显示的条目必须留在缓存里，跳过它继续淘汰更旧的条目。
            if protected.is_some_and(|key| &self.entries[index].0 == key) {
                index += 1;
                continue;
            }
            pixels -= (self.cost)(&self.entries[index].1);
            self.entries.remove(index);
        }
    }

    pub fn remove(&mut self, key: &K) {
        self.entries.retain(|(entry, _)| entry != key);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// 以路径为键、像素数为成本的测试值。
    #[derive(Clone, Debug)]
    struct Item {
        pixels: usize,
    }

    fn item(pixels: usize) -> Item {
        Item { pixels }
    }

    #[test]
    fn oldest_entry_is_evicted_after_capacity() {
        let mut cache = BudgetCache::new(3, usize::MAX, |value: &Item| value.pixels);
        for name in ["a", "b", "c"] {
            cache.insert(PathBuf::from(name), item(10), Some(&PathBuf::from("c")));
        }
        cache.insert(PathBuf::from("d"), item(10), Some(&PathBuf::from("d")));

        assert!(!cache.contains(&PathBuf::from("a")));
        assert!(cache.contains(&PathBuf::from("b")));
        assert!(cache.contains(&PathBuf::from("c")));
        assert!(cache.contains(&PathBuf::from("d")));
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn cache_hit_refreshes_recency() {
        let mut cache = BudgetCache::new(3, usize::MAX, |value: &Item| value.pixels);
        for name in ["a", "b", "c"] {
            cache.insert(PathBuf::from(name), item(10), Some(&PathBuf::from("c")));
        }

        assert!(cache.get(&PathBuf::from("a")).is_some());
        cache.insert(PathBuf::from("d"), item(10), Some(&PathBuf::from("d")));

        assert!(!cache.contains(&PathBuf::from("b")));
        assert!(cache.contains(&PathBuf::from("a")));
        assert!(cache.contains(&PathBuf::from("c")));
        assert!(cache.contains(&PathBuf::from("d")));
    }

    #[test]
    fn current_image_is_never_evicted() {
        let mut cache = BudgetCache::new(3, usize::MAX, |value: &Item| value.pixels);

        // a 是最旧的一条，但它是当前图片，必须保留。
        for name in ["a", "b", "c"] {
            cache.insert(PathBuf::from(name), item(10), Some(&PathBuf::from("a")));
        }
        cache.insert(PathBuf::from("d"), item(10), Some(&PathBuf::from("a")));

        assert!(cache.contains(&PathBuf::from("a")));
        assert!(!cache.contains(&PathBuf::from("b")));
        assert!(cache.contains(&PathBuf::from("c")));
        assert!(cache.contains(&PathBuf::from("d")));
    }

    #[test]
    fn pixel_budget_evicts_oldest() {
        let mut cache = BudgetCache::new(9, 10, |value: &Item| value.pixels);

        cache.insert(PathBuf::from("a"), item(5), Some(&PathBuf::from("c")));
        cache.insert(PathBuf::from("b"), item(5), Some(&PathBuf::from("c")));
        cache.insert(PathBuf::from("c"), item(5), Some(&PathBuf::from("c")));

        assert!(!cache.contains(&PathBuf::from("a")));
        assert!(cache.contains(&PathBuf::from("b")));
        assert!(cache.contains(&PathBuf::from("c")));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn missing_path_is_not_cached() {
        let cache = BudgetCache::<PathBuf, Item>::new(3, 100, |value| value.pixels);
        assert!(!cache.contains(&PathBuf::from("missing")));
    }

    #[test]
    fn remove_drops_only_the_requested_entry() {
        let mut cache = BudgetCache::new(3, usize::MAX, |value: &Item| value.pixels);
        for name in ["a", "b"] {
            cache.insert(PathBuf::from(name), item(10), Some(&PathBuf::from("a")));
        }

        cache.remove(&PathBuf::from("b"));

        assert!(cache.contains(&PathBuf::from("a")));
        assert!(!cache.contains(&PathBuf::from("b")));
    }

    #[test]
    fn lru_cache_evicts_by_entry_count() {
        let mut cache = LruCache::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        assert_eq!(cache.get(&"a"), Some(1));
        cache.insert("c", 3);

        assert!(cache.contains(&"a"));
        assert!(!cache.contains(&"b"));
        assert_eq!(cache.len(), 2);
    }
}
