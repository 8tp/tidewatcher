use std::{
    cmp::Ordering,
    io,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use sysinfo::Signal;

use crate::{
    config::AppConfig,
    metrics::{DashboardMetrics, MetricsOptions, ProcessSnapshot, Snapshot},
    theme::{find_theme_index, Theme, THEMES},
};

const DEFAULT_REFRESH_MS: u64 = 2_000;
const MIN_REFRESH_MS: u64 = 250;
const MAX_REFRESH_MS: u64 = 10_000;
const REFRESH_STEP_MS: u64 = 250;
const DEFAULT_ANIMATION_MS: u64 = 180;
const MIN_ANIMATION_MS: u64 = 80;
const MAX_ANIMATION_MS: u64 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusPanel {
    Cpu,
    Memory,
    Disk,
    Network,
    Processes,
}

impl FocusPanel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Disk => "disk",
            Self::Network => "network",
            Self::Processes => "processes",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Cpu => Self::Memory,
            Self::Memory => Self::Disk,
            Self::Disk => Self::Network,
            Self::Network => Self::Processes,
            Self::Processes => Self::Cpu,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Cpu => Self::Processes,
            Self::Memory => Self::Cpu,
            Self::Disk => Self::Memory,
            Self::Network => Self::Disk,
            Self::Processes => Self::Network,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessSort {
    Cpu,
    Memory,
    Io,
    Name,
}

impl ProcessSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Io => "i/o",
            Self::Name => "name",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        let normalized = value
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>();

        match normalized.as_str() {
            "cpu" => Some(Self::Cpu),
            "memory" | "mem" => Some(Self::Memory),
            "io" | "disk" => Some(Self::Io),
            "name" => Some(Self::Name),
            _ => None,
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Cpu => Self::Memory,
            Self::Memory => Self::Io,
            Self::Io => Self::Name,
            Self::Name => Self::Cpu,
        }
    }
}

pub struct App {
    metrics: DashboardMetrics,
    focus: FocusPanel,
    theme_index: usize,
    refresh_interval_ms: u64,
    animation_interval_ms: u64,
    started_at: Instant,
    last_refresh: Instant,
    should_quit: bool,
    process_sort: ProcessSort,
    selected_process_pid: Option<u32>,
    process_detail_open: bool,
    status_message: Option<String>,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let theme_index = find_theme_index(&config.theme).unwrap_or(0);
        let refresh_interval_ms = config
            .refresh_interval_ms
            .clamp(MIN_REFRESH_MS, MAX_REFRESH_MS);
        let animation_interval_ms = config
            .animation_interval_ms
            .clamp(MIN_ANIMATION_MS, MAX_ANIMATION_MS);
        let process_sort =
            ProcessSort::from_value(&config.process_sort).unwrap_or(ProcessSort::Cpu);
        let metrics = DashboardMetrics::new(MetricsOptions {
            history_capacity: config.history_capacity,
            disk_limit: config.disk_limit,
            network_limit: config.network_limit,
        });

        let mut app = Self {
            metrics,
            focus: FocusPanel::Cpu,
            theme_index,
            refresh_interval_ms: if refresh_interval_ms == 0 {
                DEFAULT_REFRESH_MS
            } else {
                refresh_interval_ms
            },
            animation_interval_ms: if animation_interval_ms == 0 {
                DEFAULT_ANIMATION_MS
            } else {
                animation_interval_ms
            },
            started_at: Instant::now(),
            last_refresh: Instant::now(),
            should_quit: false,
            process_sort,
            selected_process_pid: None,
            process_detail_open: false,
            status_message: None,
        };
        app.sync_process_selection();
        app
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn theme(&self) -> Theme {
        THEMES[self.theme_index]
    }

    pub fn focus(&self) -> FocusPanel {
        self.focus
    }

    pub fn refresh_interval(&self) -> Duration {
        Duration::from_millis(self.refresh_interval_ms)
    }

    pub fn snapshot(&self) -> &Snapshot {
        self.metrics.snapshot()
    }

    pub fn animation_frame(&self) -> usize {
        (self.started_at.elapsed().as_millis() / u128::from(self.animation_interval_ms)) as usize
    }

    pub fn cpu_history(&self) -> &[u64] {
        self.metrics.cpu_history()
    }

    pub fn memory_history(&self) -> &[u64] {
        self.metrics.memory_history()
    }

    pub fn is_selected(&self, panel: FocusPanel) -> bool {
        self.focus == panel
    }

    pub fn process_sort(&self) -> ProcessSort {
        self.process_sort
    }

    pub fn process_detail_open(&self) -> bool {
        self.process_detail_open
    }

    pub fn status_message(&self) -> Option<&str> {
        self.status_message.as_deref()
    }

    pub fn process_count(&self) -> usize {
        self.snapshot().processes.len()
    }

    pub fn selected_process_pid(&self) -> Option<u32> {
        self.selected_process_pid
    }

    pub fn selected_process(&self) -> Option<&ProcessSnapshot> {
        let pid = self.selected_process_pid?;
        self.snapshot()
            .processes
            .iter()
            .find(|process| process.pid == pid)
    }

    pub fn sorted_processes(&self) -> Vec<&ProcessSnapshot> {
        let mut processes = self.snapshot().processes.iter().collect::<Vec<_>>();
        processes.sort_by(|left, right| compare_processes(left, right, self.process_sort));
        processes
    }

    pub fn refresh_if_due(&mut self) {
        let elapsed = self.last_refresh.elapsed();

        if elapsed >= self.refresh_interval() {
            self.metrics.refresh(elapsed);
            self.last_refresh = Instant::now();
            self.sync_process_selection();
        }
    }

    pub fn handle_events(&mut self) -> io::Result<()> {
        if !event::poll(self.time_until_next_tick())? {
            return Ok(());
        }

        match event::read()? {
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                self.handle_key_event(key)
            }
            _ => Ok(()),
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> io::Result<()> {
        if self.process_detail_open {
            self.handle_process_detail_key(key)
        } else {
            self.handle_standard_key(key)
        }
    }

    fn handle_standard_key(&mut self, key: KeyEvent) -> io::Result<()> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => self.should_quit = true,
            KeyCode::Left | KeyCode::BackTab => self.focus = self.focus.previous(),
            KeyCode::Right | KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::Up => {
                if self.focus == FocusPanel::Processes {
                    self.move_process_selection(-1);
                } else {
                    self.focus = self.focus.previous();
                }
            }
            KeyCode::Down => {
                if self.focus == FocusPanel::Processes {
                    self.move_process_selection(1);
                } else {
                    self.focus = self.focus.next();
                }
            }
            KeyCode::Enter if self.focus == FocusPanel::Processes => {
                self.process_detail_open = self.selected_process_pid.is_some();
            }
            KeyCode::Char('s') if self.focus == FocusPanel::Processes => self.cycle_process_sort(),
            KeyCode::Char('t') => self.theme_index = (self.theme_index + 1) % THEMES.len(),
            KeyCode::Char('+') | KeyCode::Char('=') => self.adjust_refresh(-1),
            KeyCode::Char('-') => self.adjust_refresh(1),
            _ => {}
        }

        Ok(())
    }

    fn handle_process_detail_key(&mut self, key: KeyEvent) -> io::Result<()> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc | KeyCode::Enter => self.process_detail_open = false,
            KeyCode::Up => self.move_process_selection(-1),
            KeyCode::Down => self.move_process_selection(1),
            KeyCode::Char('s') => self.cycle_process_sort(),
            KeyCode::Char('x') => self.send_signal_to_selected(Signal::Term),
            KeyCode::Char('k') => self.send_signal_to_selected(Signal::Kill),
            _ => {}
        }

        Ok(())
    }

    fn cycle_process_sort(&mut self) {
        self.process_sort = self.process_sort.next();
        self.status_message = Some(format!("process sort: {}", self.process_sort.label()));
        self.sync_process_selection();
    }

    fn move_process_selection(&mut self, delta: isize) {
        let ordered_ids = self
            .sorted_processes()
            .into_iter()
            .map(|process| process.pid)
            .collect::<Vec<_>>();
        if ordered_ids.is_empty() {
            self.selected_process_pid = None;
            return;
        }

        let current_index = self
            .selected_process_pid
            .and_then(|pid| ordered_ids.iter().position(|current| *current == pid))
            .unwrap_or(0);
        let next_index = if delta.is_negative() {
            current_index.saturating_sub(delta.unsigned_abs())
        } else {
            current_index
                .saturating_add(delta as usize)
                .min(ordered_ids.len() - 1)
        };

        self.selected_process_pid = Some(ordered_ids[next_index]);
    }

    fn send_signal_to_selected(&mut self, signal: Signal) {
        let Some((pid, name)) = self
            .selected_process()
            .map(|process| (process.pid, process.name.clone()))
        else {
            self.status_message = Some("no process selected".to_string());
            return;
        };

        self.status_message = Some(
            match self
                .metrics
                .send_signal(pid, signal, self.refresh_interval())
            {
                Ok(()) => format!("sent {:?} to {} ({pid})", signal, name),
                Err(error) => error,
            },
        );
        self.last_refresh = Instant::now();
        self.sync_process_selection();
    }

    fn sync_process_selection(&mut self) {
        let ordered_ids = self
            .sorted_processes()
            .into_iter()
            .map(|process| process.pid)
            .collect::<Vec<_>>();

        if ordered_ids.is_empty() {
            self.selected_process_pid = None;
            self.process_detail_open = false;
            return;
        }

        if let Some(pid) = self.selected_process_pid {
            if ordered_ids.contains(&pid) {
                return;
            }
        }

        self.selected_process_pid = ordered_ids.first().copied();
    }

    fn adjust_refresh(&mut self, direction: i64) {
        let current = self.refresh_interval_ms as i64;
        let next = current + direction * REFRESH_STEP_MS as i64;
        self.refresh_interval_ms = next.clamp(MIN_REFRESH_MS as i64, MAX_REFRESH_MS as i64) as u64;
    }

    fn time_until_next_refresh(&self) -> Duration {
        self.refresh_interval()
            .saturating_sub(self.last_refresh.elapsed())
    }

    fn time_until_next_animation(&self) -> Duration {
        let elapsed_ms = self.started_at.elapsed().as_millis() as u64;
        let remainder = elapsed_ms % self.animation_interval_ms;
        let wait_ms = if remainder == 0 {
            self.animation_interval_ms
        } else {
            self.animation_interval_ms - remainder
        };
        Duration::from_millis(wait_ms)
    }

    fn time_until_next_tick(&self) -> Duration {
        self.time_until_next_refresh()
            .min(self.time_until_next_animation())
    }
}

fn compare_processes(
    left: &ProcessSnapshot,
    right: &ProcessSnapshot,
    sort: ProcessSort,
) -> Ordering {
    match sort {
        ProcessSort::Cpu => compare_f32(right.cpu_usage, left.cpu_usage)
            .then_with(|| right.memory.cmp(&left.memory))
            .then_with(|| right.io_activity().cmp(&left.io_activity()))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.pid.cmp(&right.pid)),
        ProcessSort::Memory => right
            .memory
            .cmp(&left.memory)
            .then_with(|| compare_f32(right.cpu_usage, left.cpu_usage))
            .then_with(|| right.io_activity().cmp(&left.io_activity()))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.pid.cmp(&right.pid)),
        ProcessSort::Io => right
            .io_activity()
            .cmp(&left.io_activity())
            .then_with(|| compare_f32(right.cpu_usage, left.cpu_usage))
            .then_with(|| right.memory.cmp(&left.memory))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.pid.cmp(&right.pid)),
        ProcessSort::Name => left
            .name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase())
            .then_with(|| compare_f32(right.cpu_usage, left.cpu_usage))
            .then_with(|| right.memory.cmp(&left.memory))
            .then_with(|| left.pid.cmp(&right.pid)),
    }
}

fn compare_f32(left: f32, right: f32) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{App, FocusPanel, ProcessSort};
    use crate::config::AppConfig;

    #[test]
    fn focus_panel_cycles_in_both_directions() {
        assert_eq!(FocusPanel::Cpu.next(), FocusPanel::Memory);
        assert_eq!(FocusPanel::Memory.next(), FocusPanel::Disk);
        assert_eq!(FocusPanel::Disk.next(), FocusPanel::Network);
        assert_eq!(FocusPanel::Network.next(), FocusPanel::Processes);
        assert_eq!(FocusPanel::Processes.next(), FocusPanel::Cpu);
        assert_eq!(FocusPanel::Cpu.previous(), FocusPanel::Processes);
        assert_eq!(FocusPanel::Memory.previous(), FocusPanel::Cpu);
        assert_eq!(FocusPanel::Disk.previous(), FocusPanel::Memory);
        assert_eq!(FocusPanel::Network.previous(), FocusPanel::Disk);
        assert_eq!(FocusPanel::Processes.previous(), FocusPanel::Network);
    }

    #[test]
    fn process_sort_parses_aliases() {
        assert_eq!(ProcessSort::from_value("cpu"), Some(ProcessSort::Cpu));
        assert_eq!(ProcessSort::from_value("MEM"), Some(ProcessSort::Memory));
        assert_eq!(ProcessSort::from_value("i/o"), Some(ProcessSort::Io));
        assert_eq!(ProcessSort::from_value("name"), Some(ProcessSort::Name));
        assert_eq!(ProcessSort::from_value("unknown"), None);
    }

    #[test]
    fn animation_tick_never_waits_longer_than_animation_interval() {
        let app = App::new(AppConfig::default());

        assert!(
            app.time_until_next_animation() <= Duration::from_millis(app.animation_interval_ms)
        );
        assert!(app.time_until_next_tick() <= app.refresh_interval());
    }
}
