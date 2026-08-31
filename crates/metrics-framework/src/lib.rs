//! Data-source-agnostic metric collection framework.
//!
//! The framework has no metric-category concept: a [`Collector`] declares a
//! flat set of [`MetricItem`]s and produces sampled [`SampleGroup`]s.
//!
//! # Data flow
//!
//! 1. `Registry::add` registers one observable instrument per item and spawns
//!    one sampling thread per collector.
//! 2. The sampling thread runs `collector.collect(&mut scratch)` on a reused
//!    buffer, then publishes it with an O(1) `swap` into the shared cache
//!    (a `parking_lot::RwLock<Vec<SampleGroup>>`).
//! 3. On every OTel collection, each instrument callback reads the cache under
//!    a read guard and observes the values whose name matches, borrowing the
//!    pre-built attributes — the observation path performs no allocation.

use opentelemetry::KeyValue;
use opentelemetry::metrics::{AsyncInstrument, Meter};
use parking_lot::RwLock;
use smallvec::SmallVec;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tracing::info;

/// A sampled measurement value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    U64(u64),
    F64(f64),
}

/// Instrument kind; the value type must match the `Number` variant produced
/// by the collector (`CounterU64`/`GaugeU64` <-> `Number::U64`, and the `F64`
/// kinds <-> `Number::F64`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    CounterU64,
    CounterF64,
    GaugeU64,
    GaugeF64,
}

/// A metric item definition. All fields are `'static` so a collector can
/// declare its items as a static array with zero runtime allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricItem {
    pub item_type: u16,
    /// Fully-qualified metric name, e.g. `"process_cpu_seconds_total"`.
    pub name: &'static str,
    pub kind: ItemKind,
    pub unit: &'static str,
    pub description: &'static str,
}

/// A single sampled value inside a [`SampleGroup`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleValue {
    pub name: &'static str,
    pub value: Number,
}

/// A batch of values sharing the same attribute set (e.g. all process metrics
/// of one pid share `pid` + `comm`). This is a data shape for sharing
/// attributes and avoiding per-value allocation — not a metric category.
pub struct SampleGroup {
    /// Attributes shared by all values in this batch, pre-built as `KeyValue`
    /// (static keys and numbers allocate nothing; one owned `String` per
    /// entity at most).
    pub attrs: SmallVec<[KeyValue; 4]>,
    /// Inline capacity covers the typical per-entity item count, so values
    /// stay in the group's buffer without a separate heap allocation.
    pub values: SmallVec<[SampleValue; 16]>,
}

impl SampleGroup {
    pub fn with_attrs(attrs: SmallVec<[KeyValue; 4]>) -> Self {
        Self {
            attrs,
            values: SmallVec::new(),
        }
    }

    pub fn push(&mut self, name: &'static str, value: Number) {
        self.values.push(SampleValue { name, value });
    }
}

/// A metric source. Data-source agnostic: it may read `/proc` (through an
/// adapter crate), query an HTTP API, or generate values in memory.
pub trait Collector: Send + Sync {
    /// Collector name, used for the sampling thread name and logs.
    fn name(&self) -> &'static str;

    /// Sampling interval.
    fn interval(&self) -> Duration;

    /// Metric definitions, returned as a static slice.
    fn items(&self) -> Vec<&MetricItem>;

    /// Perform one sampling pass, appending the produced groups to `out`.
    ///
    /// Implementations must reuse internal buffers across calls (the
    /// framework reuses `out` as well) and must not allocate in hot paths.
    fn collect(&mut self, out: &mut Vec<SampleGroup>);
}

struct Entry {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

/// Collection runtime: registers instruments and drives one sampling thread
/// per collector.
#[derive(Default)]
pub struct Registry {
    entries: Vec<Entry>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a collector: build one observable instrument per item, then
    /// start its sampling thread.
    ///
    /// A collector without metric items is skipped with a warning: no
    /// instruments and no sampling thread are created.
    pub fn add(&mut self, collector: Box<dyn Collector>, meter: &Meter) -> std::io::Result<()> {
        let name = collector.name();

        if collector.items().is_empty() {
            tracing::warn!("collector {name} has no metric items; skipped registration");
            return Ok(());
        }

        let cache = Arc::new(RwLock::new(Vec::new()));

        for item in collector.items() {
            register_item(meter, item, cache.clone());
        }

        let stop = Arc::new(AtomicBool::new(false));
        let handle = std::thread::Builder::new().name(name.to_string()).spawn({
            let cache = cache.clone();
            let stop = stop.clone();
            let mut collector = collector;
            let interval = collector.interval();
            move || {
                // Reused across ticks: no per-tick `Vec` allocation after
                // the capacity stabilizes.
                let mut scratch: Vec<SampleGroup> = Vec::new();
                while !stop.load(Ordering::SeqCst) {
                    let started = Instant::now();
                    sample_once(&mut *collector, &cache, &mut scratch);
                    let elapsed = started.elapsed();
                    if elapsed < interval {
                        std::thread::sleep(interval - elapsed);
                    }
                }
                info!("collector exited: {}", collector.name());
            }
        })?;

        self.entries.push(Entry {
            stop,
            handle: Some(handle),
        });
        Ok(())
    }

    /// Stop all sampling threads and join them.
    pub fn shutdown(&mut self) {
        for entry in &self.entries {
            entry.stop.store(true, Ordering::SeqCst);
        }
        for entry in &mut self.entries {
            if let Some(handle) = entry.handle.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// One sampling tick: clear the reused scratch buffer, run `collect`, then
/// publish with an O(1) `swap` (no copy, no fresh allocation).
fn sample_once(
    collector: &mut dyn Collector,
    cache: &RwLock<Vec<SampleGroup>>,
    scratch: &mut Vec<SampleGroup>,
) {
    scratch.clear();
    collector.collect(scratch);
    let mut guard = cache.write();
    std::mem::swap(&mut *guard, scratch);
}

fn register_item(meter: &Meter, item: &MetricItem, cache: Arc<RwLock<Vec<SampleGroup>>>) {
    // The callback closure is 'static; only the 'static name is captured.
    let name = item.name;
    match item.kind {
        ItemKind::CounterU64 => {
            meter
                .u64_observable_counter(name)
                .with_unit(item.unit)
                .with_description(item.description)
                .with_callback(move |inst: &dyn AsyncInstrument<u64>| {
                    observe_u64(inst, &cache, name)
                })
                .build();
        }
        ItemKind::CounterF64 => {
            meter
                .f64_observable_counter(name)
                .with_unit(item.unit)
                .with_description(item.description)
                .with_callback(move |inst: &dyn AsyncInstrument<f64>| {
                    observe_f64(inst, &cache, name)
                })
                .build();
        }
        ItemKind::GaugeU64 => {
            meter
                .u64_observable_gauge(name)
                .with_unit(item.unit)
                .with_description(item.description)
                .with_callback(move |inst: &dyn AsyncInstrument<u64>| {
                    observe_u64(inst, &cache, name)
                })
                .build();
        }
        ItemKind::GaugeF64 => {
            meter
                .f64_observable_gauge(name)
                .with_unit(item.unit)
                .with_description(item.description)
                .with_callback(move |inst: &dyn AsyncInstrument<f64>| {
                    observe_f64(inst, &cache, name)
                })
                .build();
        }
    }
}

fn observe_u64(
    inst: &dyn AsyncInstrument<u64>,
    cache: &RwLock<Vec<SampleGroup>>,
    name: &'static str,
) {
    let guard = cache.read();
    for group in guard.iter() {
        for value in group.values.iter() {
            if value.name == name
                && let Number::U64(v) = value.value
            {
                inst.observe(v, &group.attrs);
            }
        }
    }
}

fn observe_f64(
    inst: &dyn AsyncInstrument<f64>,
    cache: &RwLock<Vec<SampleGroup>>,
    name: &'static str,
) {
    let guard = cache.read();
    for group in guard.iter() {
        for value in group.values.iter() {
            if value.name == name
                && let Number::F64(v) = value.value
            {
                inst.observe(v, &group.attrs);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::Key;
    use opentelemetry::metrics::{MeterProvider, NoopMeterProvider};
    use smallvec::smallvec;
    use std::sync::atomic::AtomicU64;

    static ITEMS: [MetricItem; 1] = [MetricItem {
        item_type: 0,
        name: "fake_count",
        kind: ItemKind::CounterU64,
        unit: "{count}",
        description: "fake test counter",
    }];

    struct FakeCollector {
        interval: Duration,
    }

    impl Collector for FakeCollector {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn interval(&self) -> Duration {
            self.interval
        }
        fn items(&self) -> Vec<&MetricItem> {
            ITEMS.iter().collect()
        }
        fn collect(&mut self, out: &mut Vec<SampleGroup>) {
            let mut group = SampleGroup::with_attrs(smallvec![KeyValue::new("pid", 42i64)]);
            group.push("fake_count", Number::U64(7));
            out.push(group);
        }
    }

    #[test]
    fn sample_once_publishes_with_swap() {
        let cache = Arc::new(RwLock::new(Vec::new()));
        let mut scratch = Vec::new();
        let mut collector = FakeCollector {
            interval: Duration::from_secs(1),
        };

        sample_once(&mut collector, &cache, &mut scratch);

        // After the swap the scratch is drained back to the caller...
        assert!(scratch.is_empty());
        // ...and the cache holds exactly the sampled group.
        let guard = cache.read();
        assert_eq!(guard.len(), 1);
        assert_eq!(guard[0].values.len(), 1);
        assert_eq!(guard[0].values[0].name, "fake_count");
        assert_eq!(guard[0].values[0].value, Number::U64(7));
        assert!(guard[0].attrs.contains(&KeyValue::new("pid", 42i64)));
        assert_eq!(guard[0].attrs[0].key, Key::from_static_str("pid"));
    }

    #[test]
    fn register_and_shutdown() {
        let provider = NoopMeterProvider::new();
        let meter = provider.meter("test");

        let mut registry = Registry::new();
        registry
            .add(
                Box::new(FakeCollector {
                    interval: Duration::from_millis(10),
                }),
                &meter,
            )
            .unwrap();

        // Let the sampling thread run a couple of ticks, then stop it.
        std::thread::sleep(Duration::from_millis(35));
        registry.shutdown();
        // Drop must also be safe afterwards.
        drop(registry);
    }

    #[test]
    fn add_empty_items_skips_thread() {
        struct EmptyCollector;

        impl Collector for EmptyCollector {
            fn name(&self) -> &'static str {
                "empty"
            }
            fn interval(&self) -> Duration {
                Duration::from_secs(1)
            }
            fn items(&self) -> Vec<&MetricItem> {
                Vec::new()
            }
            fn collect(&mut self, _out: &mut Vec<SampleGroup>) {}
        }

        let provider = NoopMeterProvider::new();
        let meter = provider.meter("test");

        let mut registry = Registry::new();
        registry.add(Box::new(EmptyCollector), &meter).unwrap();

        // No sampling thread was spawned.
        assert!(registry.entries.is_empty());
    }

    #[test]
    fn observe_filters_by_name_and_type() {
        let cache = Arc::new(RwLock::new(Vec::new()));

        let mut matching = SampleGroup::with_attrs(SmallVec::new());
        matching.push("fake_count", Number::U64(3));
        let mut wrong_type = SampleGroup::with_attrs(SmallVec::new());
        wrong_type.push("fake_count", Number::F64(9.0));
        let mut other_name = SampleGroup::with_attrs(SmallVec::new());
        other_name.push("other", Number::U64(100));

        let mut scratch = vec![matching, wrong_type, other_name];
        {
            let mut guard = cache.write();
            std::mem::swap(&mut *guard, &mut scratch);
        } // write guard must be released before observing (parking_lot is not reentrant)

        let inst = RecordingInstrument::default();
        observe_u64(&inst, &cache, "fake_count");

        // Only the u64 value of the matching name is observed.
        assert_eq!(inst.observed.load(Ordering::Relaxed), 3);
    }

    /// Minimal recording instrument for exercising callback loops.
    #[derive(Default)]
    struct RecordingInstrument {
        observed: AtomicU64,
    }
    impl AsyncInstrument<u64> for RecordingInstrument {
        fn observe(&self, measurement: u64, _attributes: &[KeyValue]) {
            self.observed.fetch_add(measurement, Ordering::Relaxed);
        }
    }
}
