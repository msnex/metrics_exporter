use metrics_framework::{Number, SampleGroup};
use opentelemetry::KeyValue;
use procfs::net::IfaceStats;
use smallvec::smallvec;
use std::collections::HashSet;

/// Filter for which network interfaces to collect.
#[derive(Default, Clone, bon::Builder)]
pub struct NetFilter {
    ifaces: Option<HashSet<String>>,
}

/// Read `/proc/net/dev` and append one sample group per matching interface.
pub fn collect_net_metrics(hostname: &KeyValue, filter: &NetFilter, out: &mut Vec<SampleGroup>) {
    let Ok(stats) = procfs::net::dev_stats() else {
        return;
    };
    collect_net_devices(hostname, filter, stats.interfaces, out);
}

fn collect_net_devices(
    hostname: &KeyValue,
    filter: &NetFilter,
    interfaces: Vec<IfaceStats>,
    out: &mut Vec<SampleGroup>,
) {
    for stats in interfaces {
        if let Some(ifaces) = filter.ifaces.as_ref()
            && !ifaces.contains(&stats.name)
        {
            continue;
        }

        let mut group = SampleGroup::with_attrs(smallvec![
            hostname.clone(),
            KeyValue::new("iface", stats.name),
        ]);
        group.push("iface_rx_bytes_total", Number::U64(stats.rx_bytes));
        group.push("iface_rx_packets_total", Number::U64(stats.rx_packets));
        group.push("iface_rx_errs_total", Number::U64(stats.rx_errs));
        group.push("iface_rx_drop_total", Number::U64(stats.rx_drop));
        group.push("iface_tx_bytes_total", Number::U64(stats.tx_bytes));
        group.push("iface_tx_packets_total", Number::U64(stats.tx_packets));
        group.push("iface_tx_errs_total", Number::U64(stats.tx_errs));
        group.push("iface_tx_drop_total", Number::U64(stats.tx_drop));
        out.push(group);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn sample_interfaces() -> Vec<IfaceStats> {
        vec![
            IfaceStats {
                name: "lo".to_string(),
                rx_bytes: 100,
                rx_packets: 200,
                rx_errs: 300,
                rx_drop: 400,
                tx_bytes: 500,
                tx_packets: 600,
                tx_errs: 700,
                tx_drop: 800,
                ..Default::default()
            },
            IfaceStats {
                name: "eth0".to_string(),
                rx_bytes: 1,
                rx_packets: 2,
                rx_errs: 3,
                rx_drop: 4,
                tx_bytes: 5,
                tx_packets: 6,
                tx_errs: 7,
                tx_drop: 8,
                ..Default::default()
            },
        ]
    }

    fn value(group: &SampleGroup, name: &str) -> Number {
        group
            .values
            .iter()
            .find(|value| value.name == name)
            .map(|value| value.value)
            .expect("metric value missing")
    }

    #[test]
    fn collect_net_devices_emits_core_metrics() {
        let hostname = KeyValue::new("hostname", Arc::from("test-host"));
        let filter = NetFilter::default();
        let mut out = Vec::new();
        collect_net_devices(&hostname, &filter, sample_interfaces(), &mut out);

        assert_eq!(out.len(), 2);
        let lo = out
            .iter()
            .find(|group| {
                group
                    .attrs
                    .iter()
                    .any(|kv| kv.key.as_str() == "iface" && kv.value.as_str() == "lo")
            })
            .expect("lo group missing");

        assert!(
            lo.attrs
                .iter()
                .any(|kv| { kv.key.as_str() == "hostname" && kv.value.as_str() == "test-host" })
        );
        assert_eq!(value(lo, "iface_rx_bytes_total"), Number::U64(100));
        assert_eq!(value(lo, "iface_rx_packets_total"), Number::U64(200));
        assert_eq!(value(lo, "iface_rx_errs_total"), Number::U64(300));
        assert_eq!(value(lo, "iface_rx_drop_total"), Number::U64(400));
        assert_eq!(value(lo, "iface_tx_bytes_total"), Number::U64(500));
        assert_eq!(value(lo, "iface_tx_packets_total"), Number::U64(600));
        assert_eq!(value(lo, "iface_tx_errs_total"), Number::U64(700));
        assert_eq!(value(lo, "iface_tx_drop_total"), Number::U64(800));
    }

    #[test]
    fn collect_net_devices_filters_exact_ifaces() {
        let hostname = KeyValue::new("hostname", Arc::from("test-host"));
        let mut ifaces = HashSet::new();
        ifaces.insert("eth0".to_string());
        let filter = NetFilter::builder().maybe_ifaces(Some(ifaces)).build();

        let mut out = Vec::new();
        collect_net_devices(&hostname, &filter, sample_interfaces(), &mut out);

        assert_eq!(out.len(), 1);
        assert!(
            out[0]
                .attrs
                .iter()
                .any(|kv| { kv.key.as_str() == "iface" && kv.value.as_str() == "eth0" })
        );
    }
}
