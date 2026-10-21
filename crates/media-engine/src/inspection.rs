//! Bounded, process-local inspection data. Stream addresses never enter storage.
use download_core::Media;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

const TTL: Duration = Duration::from_secs(120);
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 32;

#[derive(Clone)]
pub(crate) struct Inspection {
    pub(crate) media: Media,
    pub(crate) transfer: Arc<[u8]>,
    pub(crate) created: Instant,
}

#[derive(Default)]
pub(crate) struct InspectionCache(HashMap<String, Inspection>);
impl InspectionCache {
    pub(crate) fn get(&mut self, url: &str) -> Option<Inspection> {
        self.0.retain(|_, entry| entry.created.elapsed() < TTL);
        self.0.get(url).cloned()
    }
    pub(crate) fn insert(&mut self, url: String, entry: Inspection) {
        self.0
            .retain(|key, entry| key != &url && entry.created.elapsed() < TTL);
        while self.0.len() >= MAX_ENTRIES
            || self
                .0
                .values()
                .map(|entry| entry.transfer.len())
                .sum::<usize>()
                + entry.transfer.len()
                > MAX_BYTES
        {
            let Some(oldest) = self
                .0
                .iter()
                .min_by_key(|(_, entry)| entry.created)
                .map(|(key, _)| key.clone())
            else {
                return;
            };
            self.0.remove(&oldest);
        }
        self.0.insert(url, entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(size: usize) -> Inspection {
        let job: download_core::Job = serde_json::from_str(include_str!(
            "../../../packages/contracts/fixtures/job.json"
        ))
        .unwrap();
        Inspection {
            media: job.media,
            transfer: vec![0; size].into(),
            created: Instant::now(),
        }
    }
    #[test]
    fn cache_bounds_bytes_entries_and_expired_stream_addresses() {
        let mut cache = InspectionCache::default();
        cache.insert("one".into(), entry(8 * 1024 * 1024));
        cache.insert("two".into(), entry(8 * 1024 * 1024));
        cache.insert("three".into(), entry(1));
        assert!(cache.get("one").is_none());
        assert!(cache.get("two").is_some());
        let mut expired = entry(1);
        expired.created -= TTL;
        cache.insert("expired".into(), expired);
        assert!(cache.get("expired").is_none());
        for index in 0..40 {
            cache.insert(format!("small-{index}"), entry(1));
        }
        assert_eq!(cache.0.len(), MAX_ENTRIES);
    }
}
