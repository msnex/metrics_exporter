//! Metric names, the single source of truth for the crate's metric contract.
//!
//! Registration (`MetricItem::name`) and sampling (`SampleGroup::push`) both
//! reference these constants, so renaming a metric requires exactly one edit.

pub(crate) const NAME_UPTIME: &str = "uptime";
pub(crate) const NAME_LOADAVG_1M: &str = "loadavg_1m";
pub(crate) const NAME_LOADAVG_5M: &str = "loadavg_5m";
pub(crate) const NAME_LOADAVG_15M: &str = "loadavg_15m";
pub(crate) const NAME_LOADAVG_RUNNING_TASKS: &str = "loadavg_running_tasks";
pub(crate) const NAME_LOADAVG_TOTAL_TASKS: &str = "loadavg_total_tasks";
pub(crate) const NAME_CPU_SECONDS_TOTAL: &str = "cpu_seconds_total";
pub(crate) const NAME_PROCESS_IO_RCHAR_TOTAL: &str = "process_io_rchar_total";
pub(crate) const NAME_PROCESS_IO_WCHAR_TOTAL: &str = "process_io_wchar_total";
pub(crate) const NAME_PROCESS_IO_READ_BYTES_TOTAL: &str = "process_io_read_bytes_total";
pub(crate) const NAME_PROCESS_IO_WRITE_BYTES_TOTAL: &str = "process_io_write_bytes_total";
pub(crate) const NAME_PROCESS_IO_CANCELLED_WRITE_BYTES_TOTAL: &str =
    "process_io_cancelled_write_bytes_total";
pub(crate) const NAME_PROCESS_IO_SYSCR_TOTAL: &str = "process_io_syscr_total";
pub(crate) const NAME_PROCESS_IO_SYSCW_TOTAL: &str = "process_io_syscw_total";
pub(crate) const NAME_PROCESS_CPU_USER_SECONDS_TOTAL: &str = "process_cpu_user_seconds_total";
pub(crate) const NAME_PROCESS_CPU_SYSTEM_SECONDS_TOTAL: &str = "process_cpu_system_seconds_total";
pub(crate) const NAME_PROCESS_MEM_VIRT_BYTES: &str = "process_mem_virt_bytes";
pub(crate) const NAME_PROCESS_MEM_RSS_BYTES: &str = "process_mem_rss_bytes";
pub(crate) const NAME_PROCESS_MEM_HWM_BYTES: &str = "process_mem_hwm_bytes";
pub(crate) const NAME_PROCESS_MEM_SHARED_BYTES: &str = "process_mem_shared_bytes";
pub(crate) const NAME_PROCESS_MEM_SWAP_BYTES: &str = "process_mem_swap_bytes";
pub(crate) const NAME_PROCESS_MEM_HUGETLB_BYTES: &str = "process_mem_hugetlb_bytes";
pub(crate) const NAME_PROCESS_FD_SIZE: &str = "process_fd_size";
pub(crate) const NAME_PROCESS_THREADS: &str = "process_threads";
pub(crate) const NAME_PROCESS_CTXT_SWITCHES_VOLUNTARY_TOTAL: &str =
    "process_ctxt_switches_voluntary_total";
pub(crate) const NAME_PROCESS_CTXT_SWITCHES_NONVOLUNTARY_TOTAL: &str =
    "process_ctxt_switches_nonvoluntary_total";
pub(crate) const NAME_IFACE_RX_BYTES_TOTAL: &str = "iface_rx_bytes_total";
pub(crate) const NAME_IFACE_RX_PACKETS_TOTAL: &str = "iface_rx_packets_total";
pub(crate) const NAME_IFACE_RX_ERRS_TOTAL: &str = "iface_rx_errs_total";
pub(crate) const NAME_IFACE_RX_DROP_TOTAL: &str = "iface_rx_drop_total";
pub(crate) const NAME_IFACE_TX_BYTES_TOTAL: &str = "iface_tx_bytes_total";
pub(crate) const NAME_IFACE_TX_PACKETS_TOTAL: &str = "iface_tx_packets_total";
pub(crate) const NAME_IFACE_TX_ERRS_TOTAL: &str = "iface_tx_errs_total";
pub(crate) const NAME_IFACE_TX_DROP_TOTAL: &str = "iface_tx_drop_total";
pub(crate) const NAME_DISK_READ_BYTES_TOTAL: &str = "disk_read_bytes_total";
pub(crate) const NAME_DISK_READS_COMPLETED_TOTAL: &str = "disk_reads_completed_total";
pub(crate) const NAME_DISK_READS_MERGED_TOTAL: &str = "disk_reads_merged_total";
pub(crate) const NAME_DISK_READ_TIME_MS_TOTAL: &str = "disk_read_time_ms_total";
pub(crate) const NAME_DISK_WRITE_BYTES_TOTAL: &str = "disk_write_bytes_total";
pub(crate) const NAME_DISK_WRITES_COMPLETED_TOTAL: &str = "disk_writes_completed_total";
pub(crate) const NAME_DISK_WRITES_MERGED_TOTAL: &str = "disk_writes_merged_total";
pub(crate) const NAME_DISK_WRITE_TIME_MS_TOTAL: &str = "disk_write_time_ms_total";
pub(crate) const NAME_DISK_IO_IN_PROGRESS: &str = "disk_io_in_progress";
pub(crate) const NAME_DISK_IO_TIME_MS_TOTAL: &str = "disk_io_time_ms_total";
pub(crate) const NAME_DISK_IO_TIME_WEIGHTED_MS_TOTAL: &str = "disk_io_time_weighted_ms_total";
pub(crate) const NAME_DISK_DISCARD_BYTES_TOTAL: &str = "disk_discard_bytes_total";
pub(crate) const NAME_DISK_DISCARDS_COMPLETED_TOTAL: &str = "disk_discards_completed_total";
pub(crate) const NAME_DISK_DISCARDS_MERGED_TOTAL: &str = "disk_discards_merged_total";
pub(crate) const NAME_DISK_DISCARD_TIME_MS_TOTAL: &str = "disk_discard_time_ms_total";
pub(crate) const NAME_DISK_FLUSH_REQUESTS_TOTAL: &str = "disk_flush_requests_total";
pub(crate) const NAME_DISK_FLUSH_TIME_MS_TOTAL: &str = "disk_flush_time_ms_total";
pub(crate) const NAME_MEM_TOTAL_BYTES: &str = "mem_total_bytes";
pub(crate) const NAME_MEM_FREE_BYTES: &str = "mem_free_bytes";
pub(crate) const NAME_MEM_AVAILABLE_BYTES: &str = "mem_available_bytes";
pub(crate) const NAME_SWAP_TOTAL_BYTES: &str = "swap_total_bytes";
pub(crate) const NAME_SWAP_FREE_BYTES: &str = "swap_free_bytes";
pub(crate) const NAME_HUGE_PAGES_TOTAL_BYTES: &str = "huge_pages_total_bytes";
pub(crate) const NAME_HUGE_PAGES_FREE_BYTES: &str = "huge_pages_free_bytes";
