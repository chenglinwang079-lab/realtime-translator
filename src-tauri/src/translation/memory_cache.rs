use std::num::NonZeroUsize;

use lru::LruCache;
use tokio::sync::Mutex;

/// 内存缓存条目
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub translated: String,
    pub engine_id: String,
    pub source_lang: String,
    pub target_lang: String,
}

/// LRU 内存缓存（线程安全）
pub struct MemoryCache {
    inner: Mutex<LruCache<String, CacheEntry>>,
}

impl MemoryCache {
    /// 创建指定容量的内存缓存
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).unwrap();
        Self {
            inner: Mutex::new(LruCache::new(cap)),
        }
    }

    /// 查询缓存
    pub async fn get(&self, key: &str) -> Option<CacheEntry> {
        let mut cache = self.inner.lock().await;
        cache.get(key).cloned()
    }

    /// 写入缓存
    pub async fn put(&self, key: String, entry: CacheEntry) {
        let mut cache = self.inner.lock().await;
        cache.put(key, entry);
    }

    /// 清空缓存
    pub async fn clear(&self) {
        let mut cache = self.inner.lock().await;
        cache.clear();
    }

    /// 当前条目数
    pub async fn len(&self) -> usize {
        let cache = self.inner.lock().await;
        cache.len()
    }
}
