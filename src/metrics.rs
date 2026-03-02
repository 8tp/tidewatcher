use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    path::Path,
    thread,
    time::Duration,
};

#[cfg(target_os = "macos")]
use libc::{
    host_statistics64, proc_pidinfo, proc_taskinfo, sysctlbyname, vm_statistics64, HOST_VM_INFO64,
    HOST_VM_INFO64_COUNT, KERN_SUCCESS, PROC_PIDTASKINFO,
};
use sysinfo::{
    CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, Networks, Pid, ProcessRefreshKind,
    ProcessesToUpdate, RefreshKind, Signal, System, MINIMUM_CPU_UPDATE_INTERVAL,
};

const DEFAULT_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug)]
pub struct MetricsOptions {
    pub history_capacity: usize,
    pub disk_limit: usize,
    pub network_limit: usize,
}

pub struct DashboardMetrics {
    collector: MetricsCollector,
    snapshot: Snapshot,
    cpu_history: History,
    memory_history: History,
    disk_series: SeriesStore,
    network_series: SeriesStore,
}

impl DashboardMetrics {
    pub fn new(options: MetricsOptions) -> Self {
        let mut collector = MetricsCollector::new(options);
        let snapshot = collector.snapshot(DEFAULT_SAMPLE_INTERVAL);
        let mut metrics = Self {
            collector,
            snapshot: Snapshot::empty(DEFAULT_SAMPLE_INTERVAL),
            cpu_history: History::with_capacity(options.history_capacity),
            memory_history: History::with_capacity(options.history_capacity),
            disk_series: SeriesStore::new(options.history_capacity),
            network_series: SeriesStore::new(options.history_capacity),
        };
        metrics.snapshot = metrics.apply_histories(snapshot);
        metrics
    }

    pub fn refresh(&mut self, elapsed: Duration) {
        let snapshot = self.collector.sample(elapsed);
        self.snapshot = self.apply_histories(snapshot);
    }

    pub fn send_signal(
        &mut self,
        pid: u32,
        signal: Signal,
        sample_interval: Duration,
    ) -> Result<(), String> {
        self.collector.send_signal(pid, signal)?;
        let snapshot = self.collector.sample(sample_interval);
        self.snapshot = self.apply_histories(snapshot);
        Ok(())
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    pub fn cpu_history(&self) -> &[u64] {
        self.cpu_history.as_slice()
    }

    pub fn memory_history(&self) -> &[u64] {
        self.memory_history.as_slice()
    }

    fn apply_histories(&mut self, mut snapshot: Snapshot) -> Snapshot {
        self.cpu_history.push_percent(snapshot.global_cpu);
        self.memory_history.push_percent(snapshot.memory_percent());
        self.disk_series.update(
            snapshot
                .disks
                .iter()
                .map(|disk| (disk.history_key().to_string(), disk.activity())),
        );
        self.network_series.update(
            snapshot
                .networks
                .iter()
                .map(|network| (network.history_key().to_string(), network.activity())),
        );

        for disk in &mut snapshot.disks {
            disk.history = self.disk_series.values_for(disk.history_key());
        }
        for network in &mut snapshot.networks {
            network.history = self.network_series.values_for(network.history_key());
        }

        snapshot
    }
}

pub struct MetricsCollector {
    system: System,
    disks: Disks,
    networks: Networks,
    #[cfg(target_os = "macos")]
    host_port: libc::mach_port_t,
    #[cfg(target_os = "macos")]
    process_cpu_times: HashMap<u32, u64>,
    #[cfg(target_os = "macos")]
    timebase_frequency: u64,
    options: MetricsOptions,
}

impl MetricsCollector {
    pub fn new(options: MetricsOptions) -> Self {
        let mut system = System::new_with_specifics(refresh_kind());
        let mut disks = Disks::new_with_refreshed_list_specifics(disk_refresh_kind());
        let mut networks = Networks::new_with_refreshed_list();

        system.refresh_processes_specifics(ProcessesToUpdate::All, true, process_refresh_kind());
        thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL.max(Duration::from_millis(200)));
        system.refresh_specifics(refresh_kind());
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, process_refresh_kind());
        disks.refresh_specifics(true, disk_refresh_kind());
        networks.refresh(true);

        Self {
            system,
            disks,
            networks,
            #[cfg(target_os = "macos")]
            host_port: unsafe {
                #[allow(deprecated)]
                {
                    libc::mach_host_self()
                }
            },
            #[cfg(target_os = "macos")]
            process_cpu_times: HashMap::new(),
            #[cfg(target_os = "macos")]
            timebase_frequency: macos_timebase_frequency().unwrap_or(1_000_000_000),
            options,
        }
    }

    pub fn sample(&mut self, sample_interval: Duration) -> Snapshot {
        self.system.refresh_specifics(refresh_kind());
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            process_refresh_kind(),
        );
        self.disks.refresh_specifics(true, disk_refresh_kind());
        self.networks.refresh(true);
        self.snapshot(sample_interval)
    }

    pub fn send_signal(&mut self, pid: u32, signal: Signal) -> Result<(), String> {
        let pid = Pid::from_u32(pid);
        let update = [pid];
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&update),
            false,
            process_refresh_kind(),
        );

        let Some(process) = self.system.process(pid) else {
            return Err(format!("process {pid} is no longer available"));
        };

        match process.kill_with(signal) {
            Some(true) => Ok(()),
            Some(false) => Err(format!("failed to send {:?} to process {pid}", signal)),
            None => Err(format!("{:?} is not supported on this platform", signal)),
        }
    }

    pub fn snapshot(&mut self, sample_interval: Duration) -> Snapshot {
        let memory = collect_memory_sample(
            &self.system,
            #[cfg(target_os = "macos")]
            self.host_port,
        );
        let mut disks = self
            .disks
            .list()
            .iter()
            .map(|disk| DiskSnapshot {
                name: os_value(disk.name()),
                mount_point: path_value(disk.mount_point()),
                total_space: disk.total_space(),
                available_space: disk.available_space(),
                read_bytes: disk.usage().read_bytes,
                written_bytes: disk.usage().written_bytes,
                history: Vec::new(),
            })
            .collect::<Vec<_>>();
        disks.sort_by(|left, right| {
            right
                .activity()
                .cmp(&left.activity())
                .then_with(|| compare_f32(right.used_percent(), left.used_percent()))
        });
        disks.truncate(self.options.disk_limit);

        let mut networks = self
            .networks
            .iter()
            .map(|(name, network)| NetworkSnapshot {
                interface: name.clone(),
                received: network.received(),
                transmitted: network.transmitted(),
                total_received: network.total_received(),
                total_transmitted: network.total_transmitted(),
                packets_received: network.packets_received(),
                packets_transmitted: network.packets_transmitted(),
                history: Vec::new(),
            })
            .collect::<Vec<_>>();
        networks.sort_by(|left, right| {
            right
                .activity()
                .cmp(&left.activity())
                .then_with(|| right.total_activity().cmp(&left.total_activity()))
        });
        if networks.iter().any(|network| network.activity() > 0) {
            networks.retain(|network| network.activity() > 0);
        }
        networks.truncate(self.options.network_limit);

        #[cfg(target_os = "macos")]
        let active_pids = self
            .system
            .processes()
            .keys()
            .map(|pid| pid.as_u32())
            .collect::<Vec<_>>();
        #[cfg(target_os = "macos")]
        self.process_cpu_times
            .retain(|pid, _| active_pids.contains(pid));

        let process_rows = self
            .system
            .processes()
            .values()
            .map(|process| {
                let command = join_os_values(process.cmd());
                (
                    process.pid().as_u32(),
                    process.parent().map(|pid| pid.as_u32()),
                    os_value(process.name()),
                    format!("{:?}", process.status()),
                    process.cpu_usage(),
                    process.memory(),
                    process.disk_usage().read_bytes,
                    process.disk_usage().written_bytes,
                    process.run_time(),
                    if command.is_empty() {
                        os_value(process.name())
                    } else {
                        command
                    },
                )
            })
            .collect::<Vec<_>>();

        let processes = process_rows
            .into_iter()
            .map(
                |(
                    pid,
                    parent_pid,
                    name,
                    status,
                    fallback_cpu_usage,
                    memory,
                    read_bytes,
                    written_bytes,
                    run_time_secs,
                    command,
                )| {
                    #[cfg(target_os = "macos")]
                    let cpu_usage = {
                        let native_cpu = self.macos_process_cpu_usage(pid, sample_interval);
                        if native_cpu > 0.0 {
                            native_cpu
                        } else {
                            fallback_cpu_usage
                        }
                    };
                    #[cfg(not(target_os = "macos"))]
                    let cpu_usage = fallback_cpu_usage;

                    ProcessSnapshot {
                        pid,
                        parent_pid,
                        name,
                        status,
                        cpu_usage,
                        memory,
                        read_bytes,
                        written_bytes,
                        run_time_secs,
                        command,
                    }
                },
            )
            .collect();

        Snapshot {
            sample_interval,
            global_cpu: self.system.global_cpu_usage(),
            cores: self
                .system
                .cpus()
                .iter()
                .enumerate()
                .map(|(index, cpu)| CoreSnapshot {
                    label: format!("cpu{index}"),
                    usage: cpu.cpu_usage(),
                })
                .collect(),
            total_memory: memory.total,
            used_memory: memory.used,
            available_memory: memory.available,
            free_memory: memory.free,
            total_swap: self.system.total_swap(),
            used_swap: self.system.used_swap(),
            disks,
            networks,
            processes,
        }
    }

    #[cfg(target_os = "macos")]
    fn macos_process_cpu_usage(&mut self, pid: u32, sample_interval: Duration) -> f32 {
        let Some(current_ticks) = macos_process_cpu_ticks(pid) else {
            return 0.0;
        };
        let previous_ticks = self.process_cpu_times.insert(pid, current_ticks);
        let Some(previous_ticks) = previous_ticks else {
            return 0.0;
        };

        macos_process_cpu_percent(
            current_ticks.saturating_sub(previous_ticks),
            sample_interval,
            self.timebase_frequency,
            self.system.cpus().len(),
        )
    }
}

fn refresh_kind() -> RefreshKind {
    RefreshKind::nothing()
        .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
        .with_memory(MemoryRefreshKind::everything())
}

fn disk_refresh_kind() -> DiskRefreshKind {
    DiskRefreshKind::nothing().with_storage().with_io_usage()
}

fn process_refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .with_memory()
        .with_cpu()
        .with_disk_usage()
        .without_tasks()
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub sample_interval: Duration,
    pub global_cpu: f32,
    pub cores: Vec<CoreSnapshot>,
    pub total_memory: u64,
    pub used_memory: u64,
    pub available_memory: u64,
    pub free_memory: u64,
    pub total_swap: u64,
    pub used_swap: u64,
    pub disks: Vec<DiskSnapshot>,
    pub networks: Vec<NetworkSnapshot>,
    pub processes: Vec<ProcessSnapshot>,
}

impl Snapshot {
    fn empty(sample_interval: Duration) -> Self {
        Self {
            sample_interval,
            global_cpu: 0.0,
            cores: Vec::new(),
            total_memory: 0,
            used_memory: 0,
            available_memory: 0,
            free_memory: 0,
            total_swap: 0,
            used_swap: 0,
            disks: Vec::new(),
            networks: Vec::new(),
            processes: Vec::new(),
        }
    }

    pub fn memory_percent(&self) -> f32 {
        percent(self.used_memory, self.total_memory)
    }

    pub fn total_disk_space(&self) -> u64 {
        self.disks.iter().map(|disk| disk.total_space).sum()
    }

    pub fn used_disk_space(&self) -> u64 {
        self.disks.iter().map(DiskSnapshot::used_space).sum()
    }

    pub fn disk_percent(&self) -> f32 {
        percent(self.used_disk_space(), self.total_disk_space())
    }

    pub fn total_network_received(&self) -> u64 {
        self.networks.iter().map(|network| network.received).sum()
    }

    pub fn total_network_transmitted(&self) -> u64 {
        self.networks
            .iter()
            .map(|network| network.transmitted)
            .sum()
    }

    pub fn sample_interval_secs(&self) -> f64 {
        self.sample_interval.as_secs_f64().max(0.001)
    }
}

#[derive(Clone, Debug)]
pub struct CoreSnapshot {
    pub label: String,
    pub usage: f32,
}

#[derive(Clone, Debug)]
pub struct DiskSnapshot {
    pub name: String,
    pub mount_point: String,
    pub total_space: u64,
    pub available_space: u64,
    pub read_bytes: u64,
    pub written_bytes: u64,
    pub history: Vec<u64>,
}

impl DiskSnapshot {
    pub fn used_space(&self) -> u64 {
        self.total_space.saturating_sub(self.available_space)
    }

    pub fn used_percent(&self) -> f32 {
        percent(self.used_space(), self.total_space)
    }

    pub fn activity(&self) -> u64 {
        self.read_bytes + self.written_bytes
    }

    pub fn label(&self) -> &str {
        if self.mount_point != "/" {
            &self.mount_point
        } else if !self.name.is_empty() {
            &self.name
        } else {
            "/"
        }
    }

    pub fn history_key(&self) -> &str {
        self.label()
    }
}

#[derive(Clone, Debug)]
pub struct NetworkSnapshot {
    pub interface: String,
    pub received: u64,
    pub transmitted: u64,
    pub total_received: u64,
    pub total_transmitted: u64,
    pub packets_received: u64,
    pub packets_transmitted: u64,
    pub history: Vec<u64>,
}

impl NetworkSnapshot {
    pub fn activity(&self) -> u64 {
        self.received + self.transmitted
    }

    pub fn total_activity(&self) -> u64 {
        self.total_received + self.total_transmitted
    }

    pub fn history_key(&self) -> &str {
        &self.interface
    }
}

#[derive(Clone, Debug)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub status: String,
    pub cpu_usage: f32,
    pub memory: u64,
    pub read_bytes: u64,
    pub written_bytes: u64,
    pub run_time_secs: u64,
    pub command: String,
}

impl ProcessSnapshot {
    pub fn io_activity(&self) -> u64 {
        self.read_bytes + self.written_bytes
    }
}

#[derive(Clone, Debug)]
struct History {
    capacity: usize,
    samples: Vec<u64>,
}

impl History {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            samples: Vec::with_capacity(capacity),
        }
    }

    fn push_percent(&mut self, value: f32) {
        self.push_value(value.clamp(0.0, 100.0).round() as u64);
    }

    fn push_value(&mut self, value: u64) {
        if self.samples.len() == self.capacity {
            self.samples.remove(0);
        }

        self.samples.push(value);
    }

    fn as_slice(&self) -> &[u64] {
        &self.samples
    }
}

#[derive(Clone, Debug)]
struct SeriesStore {
    capacity: usize,
    series: HashMap<String, History>,
}

impl SeriesStore {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            series: HashMap::new(),
        }
    }

    fn update<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = (String, u64)>,
    {
        for (key, value) in items {
            self.series
                .entry(key)
                .or_insert_with(|| History::with_capacity(self.capacity))
                .push_value(value);
        }
    }

    fn values_for(&self, key: &str) -> Vec<u64> {
        self.series
            .get(key)
            .map(|history| history.as_slice().to_vec())
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MemorySample {
    total: u64,
    used: u64,
    available: u64,
    free: u64,
}

fn collect_memory_sample(
    system: &System,
    #[cfg(target_os = "macos")] host_port: libc::mach_port_t,
) -> MemorySample {
    #[cfg(target_os = "macos")]
    if let Some(memory) = macos_memory_sample(system.total_memory(), host_port) {
        return memory;
    }

    memory_sample_from_parts(
        system.total_memory(),
        system.available_memory(),
        system.free_memory(),
    )
}

fn memory_sample_from_parts(total: u64, available: u64, free: u64) -> MemorySample {
    let available = available.min(total);
    let free = free.min(available);

    MemorySample {
        total,
        used: total.saturating_sub(available),
        available,
        free,
    }
}

#[cfg(target_os = "macos")]
fn macos_process_cpu_ticks(pid: u32) -> Option<u64> {
    let mut info = std::mem::MaybeUninit::<proc_taskinfo>::uninit();
    let size = std::mem::size_of::<proc_taskinfo>() as i32;
    let result = unsafe {
        proc_pidinfo(
            pid as i32,
            PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr() as *mut _,
            size,
        )
    };

    if result != size {
        return None;
    }

    let info = unsafe { info.assume_init() };
    Some(info.pti_total_user.saturating_add(info.pti_total_system))
}

#[cfg(target_os = "macos")]
fn macos_timebase_frequency() -> Option<u64> {
    let mut value = 0u64;
    let mut size = std::mem::size_of::<u64>();
    let name = b"hw.tbfrequency\0";
    let result = unsafe {
        sysctlbyname(
            name.as_ptr().cast(),
            (&mut value as *mut u64).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };

    (result == 0 && value > 0).then_some(value)
}

#[cfg(target_os = "macos")]
fn macos_process_cpu_percent(
    delta_ticks: u64,
    sample_interval: Duration,
    timebase_frequency: u64,
    logical_cpus: usize,
) -> f32 {
    if delta_ticks == 0 || timebase_frequency == 0 {
        return 0.0;
    }

    let elapsed_secs = sample_interval.as_secs_f64().max(0.001);
    let cpu_secs = delta_ticks as f64 / timebase_frequency as f64;
    let max_cpu = logical_cpus.max(1) as f64 * 100.0;

    ((cpu_secs / elapsed_secs) * 100.0).clamp(0.0, max_cpu) as f32
}

#[cfg(target_os = "macos")]
fn macos_memory_sample(total_memory: u64, host_port: libc::mach_port_t) -> Option<MemorySample> {
    let page_size = unsafe { libc::vm_page_size as u64 };
    if page_size == 0 {
        return None;
    }

    let mut count = HOST_VM_INFO64_COUNT;
    let mut stat = unsafe { std::mem::zeroed::<vm_statistics64>() };
    let result = unsafe {
        host_statistics64(
            host_port,
            HOST_VM_INFO64,
            &mut stat as *mut vm_statistics64 as *mut _,
            &mut count,
        )
    };

    if result != KERN_SUCCESS {
        return None;
    }

    Some(macos_memory_sample_from_vm_stats(
        total_memory,
        page_size,
        u64::from(stat.active_count),
        u64::from(stat.inactive_count),
        u64::from(stat.free_count),
        u64::from(stat.speculative_count),
    ))
}

#[cfg(target_os = "macos")]
fn macos_memory_sample_from_vm_stats(
    total_memory: u64,
    page_size: u64,
    active: u64,
    inactive: u64,
    free: u64,
    speculative: u64,
) -> MemorySample {
    let available = active
        .saturating_add(inactive)
        .saturating_add(free)
        .saturating_add(speculative)
        .saturating_mul(page_size);
    let free = free.saturating_sub(speculative).saturating_mul(page_size);

    memory_sample_from_parts(total_memory, available, free)
}

fn percent(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64 * 100.0) as f32
    }
}

fn os_value(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
}

fn path_value(value: &Path) -> String {
    value.display().to_string()
}

fn compare_f32(left: f32, right: f32) -> std::cmp::Ordering {
    left.partial_cmp(&right)
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn join_os_values(values: &[OsString]) -> String {
    const MAX_COMMAND_LENGTH: usize = 120;
    let mut command = String::new();

    for value in values {
        let piece = value.to_string_lossy();
        if command.len() + piece.len() + 1 > MAX_COMMAND_LENGTH {
            if !command.is_empty() {
                command.push_str(" ...");
            }
            break;
        }
        if !command.is_empty() {
            command.push(' ');
        }
        command.push_str(&piece);
    }

    command
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        memory_sample_from_parts, DiskSnapshot, History, NetworkSnapshot, SeriesStore, Snapshot,
    };

    #[test]
    fn history_keeps_only_the_latest_values() {
        let mut history = History::with_capacity(3);
        history.push_percent(10.0);
        history.push_percent(20.0);
        history.push_percent(30.0);
        history.push_percent(40.0);

        assert_eq!(history.as_slice(), &[20, 30, 40]);
    }

    #[test]
    fn history_clamps_values_to_percent_range() {
        let mut history = History::with_capacity(2);
        history.push_percent(-10.0);
        history.push_percent(120.0);

        assert_eq!(history.as_slice(), &[0, 100]);
    }

    #[test]
    fn disk_percent_is_aggregated_from_visible_disks() {
        let snapshot = Snapshot {
            sample_interval: Duration::from_secs(2),
            global_cpu: 0.0,
            cores: Vec::new(),
            total_memory: 0,
            used_memory: 0,
            available_memory: 0,
            free_memory: 0,
            total_swap: 0,
            used_swap: 0,
            disks: vec![
                DiskSnapshot {
                    name: "disk0".into(),
                    mount_point: "/".into(),
                    total_space: 100,
                    available_space: 25,
                    read_bytes: 10,
                    written_bytes: 5,
                    history: Vec::new(),
                },
                DiskSnapshot {
                    name: "disk1".into(),
                    mount_point: "/Volumes/Data".into(),
                    total_space: 100,
                    available_space: 50,
                    read_bytes: 20,
                    written_bytes: 10,
                    history: Vec::new(),
                },
            ],
            networks: vec![NetworkSnapshot {
                interface: "en0".into(),
                received: 100,
                transmitted: 50,
                total_received: 1_000,
                total_transmitted: 500,
                packets_received: 0,
                packets_transmitted: 0,
                history: Vec::new(),
            }],
            processes: Vec::new(),
        };

        assert_eq!(snapshot.used_disk_space(), 125);
        assert_eq!(snapshot.total_disk_space(), 200);
        assert_eq!(snapshot.disk_percent().round() as u64, 63);
        assert_eq!(snapshot.total_network_received(), 100);
        assert_eq!(snapshot.total_network_transmitted(), 50);
    }

    #[test]
    fn series_store_tracks_each_key_independently() {
        let mut store = SeriesStore::new(3);
        store.update([("en0".to_string(), 10), ("en1".to_string(), 5)]);
        store.update([("en0".to_string(), 20)]);

        assert_eq!(store.values_for("en0"), vec![10, 20]);
        assert_eq!(store.values_for("en1"), vec![5]);
    }

    #[test]
    fn memory_sample_uses_complementary_used_and_available_values() {
        let sample = memory_sample_from_parts(16_000, 7_000, 128);

        assert_eq!(sample.total, 16_000);
        assert_eq!(sample.used, 9_000);
        assert_eq!(sample.available, 7_000);
        assert_eq!(sample.free, 128);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_memory_sample_respects_vm_page_counts() {
        let sample = super::macos_memory_sample_from_vm_stats(
            17_179_869_184,
            16_384,
            229_497,
            228_002,
            5_580,
            292,
        );

        assert_eq!(sample.available, 7_591_870_464);
        assert_eq!(sample.free, 86_638_592);
        assert_eq!(sample.used, 9_587_998_720);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_process_cpu_percent_uses_tbfrequency_ticks() {
        let cpu = super::macos_process_cpu_percent(
            12_000_000,
            Duration::from_millis(500),
            24_000_000,
            12,
        );

        assert!((cpu - 100.0).abs() < 0.5);
    }
}
