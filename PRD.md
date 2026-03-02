# Product Requirements Document (PRD) for Tidewatcher

## 1. Product Overview

### 1.1 Product Name
Tidewatcher

### 1.2 Product Description
Tidewatcher is a real-time system monitoring tool designed as a Terminal User Interface (TUI) application. It provides users with an aesthetically pleasing and interactive dashboard to monitor key system metrics including CPU usage, memory consumption, disk I/O, network activity, and the busiest local processes. Built using Ratatui, Tidewatcher aims to offer a modern, customizable alternative to traditional tools like htop, with live-updating charts, theme-aware ASCII motion, and a focus on visual appeal and usability in terminal environments.

The core concept is to transform raw system data into dynamic, wave-like visualizations (inspired by the "tide" theme) that update in real-time, making system monitoring engaging and intuitive without sacrificing performance.

### 1.3 Objectives
- Provide a lightweight, efficient system monitor that runs seamlessly in terminal sessions.
- Enhance user experience with beautiful, customizable UI elements and charts.
- Support cross-platform compatibility (Linux, macOS, Windows via WSL or native terminals).
- Encourage open-source contributions by keeping the codebase modular and well-documented.
- Achieve better resource efficiency compared to graphical alternatives like GNOME System Monitor or Task Manager.

### 1.4 Target Audience
- System administrators and DevOps engineers who need quick insights into server performance.
- Developers and power users monitoring local machines during development or debugging.
- Enthusiasts interested in terminal-based tools and Rust programming.
- Users of command-line environments (e.g., SSH sessions, headless servers) where GUI tools are impractical.

### 1.5 Scope
- **In Scope**: Core monitoring of CPU, memory, disk, and network; TUI rendering with Ratatui; basic customization options.
- **Out of Scope**: Advanced alerting/notifications, integration with external services (e.g., Prometheus), hardware-specific sensors (e.g., GPU monitoring in initial versions).

## 2. Features

### 2.1 Core Features
- **Real-Time Metric Display**:
  - CPU: Per-core usage percentages, whole-machine average usage, and top-process CPU ranking. Temperature remains optional for future iterations.
  - Memory: Total/in-use/available/free RAM, swap usage, and availability semantics that match the underlying operating system where possible.
  - Disk: Read/write throughput and usage per mount point. I/O latency remains future work.
  - Network: Upload/download throughput and interface-specific stats, with packet-level details reserved for later if needed.
- **Live-Updating Charts**:
  - Sparkline or waveform charts for CPU, memory, disk, and network activity, updating every 1-5 seconds (configurable).
  - Historical data visualization over a sliding window for CPU and memory plus per-mount and per-interface series.
- **Interactive Navigation**:
  - Keyboard-driven controls (e.g., arrow keys for section focus, 'q' to quit).
  - Sorting/filtering of detailed views (e.g., top processes by CPU, memory, I/O, or name).
  - Dedicated process detail view with quick signal actions.
  - Zoom/pan on charts remains future work.
- **Theming and Customization**:
  - Pre-built themes (including ocean, sakura, ember, and winter variants) plus theme-aware ASCII tide scenes.
  - Configurable via TOML file: theme, refresh rates, animation speed, history depth, process sort, and dashboard density.
- **Process Management**:
  - List of top processes with detail view and `TERM` / `KILL` capabilities. Renice remains future work.

### 2.2 Advanced Features (Future Iterations)
- Export metrics to CSV/JSON for logging.
- Plugin system for extending metrics (e.g., battery status for laptops).
- Multi-machine monitoring via lightweight server-client mode.

### 2.3 Current Implementation Snapshot
- Implemented now:
  - CPU, memory, disk, network, and process panels in a single dashboard.
  - Sortable process table, process detail modal, and `TERM` / `KILL` actions.
  - Per-core CPU view plus rolling history for CPU, memory, mounts, and interfaces.
  - Built-in themes and theme-aware ASCII tide animation.
  - TOML configuration for refresh cadence, animation cadence, history depth, theme, process sort, and panel density.
  - Native macOS memory and per-process CPU sampling for better metric accuracy.
- Planned next:
  - Temperature support where available.
  - Chart zoom/pan.
  - Renice support.
  - Export, plugins, and multi-machine modes.

## 3. User Stories

- **As a system admin**, I want to launch Tidewatcher in a terminal and immediately see an overview dashboard so I can quickly assess system health.
- **As a developer**, I want customizable refresh intervals and themes so the tool fits my workflow without overwhelming my terminal.
- **As a power user**, I want interactive charts to drill down into spikes in CPU or network usage to identify issues.
- **As a beginner**, I want intuitive keyboard shortcuts and help menus so I can use the tool without extensive documentation.
- **As an open-source contributor**, I want modular code structure so I can add new metrics or UI components easily.

## 4. Technical Requirements

### 4.1 Technology Stack
- **Language**: Rust (for performance and safety).
- **UI Framework**: Ratatui (for TUI rendering) + Crossterm (for terminal backend).
- **System Metrics Libraries**: sysinfo crate for broad cross-platform collection plus platform-specific fallbacks where needed for accuracy (for example, Mach VM counters and libproc-derived process CPU on macOS).
- **Charting**: Integrated with Ratatui's built-in widgets; custom widgets for waveforms.
- **Dependencies**: Minimal external crates to keep binary size small (<10MB).

### 4.2 System Requirements
- **Operating Systems**: Linux (primary), macOS, Windows (via compatible terminals).
- **Hardware**: Minimal; should run on low-spec machines (e.g., Raspberry Pi).
- **Installation**: Via Cargo (Rust package manager), Homebrew, or pre-built binaries on GitHub releases.

### 4.3 Data Handling
- Metrics fetched via system calls (e.g., /proc on Linux) with no persistent storage unless exporting.
- On macOS, memory and per-process CPU may use native system APIs instead of sysinfo-derived aggregates to preserve accuracy.
- Ensure no sensitive data (e.g., process arguments) is displayed by default; opt-in for details.

## 5. Non-Functional Requirements

### 5.1 Performance
- Refresh rate: Default 2 seconds; low CPU overhead (<1% usage during idle).
- Memory footprint: <50MB.
- Startup time: <1 second.

### 5.2 Usability
- Responsive UI: Handle terminal resizes gracefully.
- Graceful density reduction: collapse non-essential detail and decorative art on narrower terminals.
- Accessibility: High-contrast themes; keyboard-only navigation.
- Error Handling: Graceful degradation if metrics unavailable (e.g., no network interfaces).

### 5.3 Security
- No network exposure by default.
- Run with user privileges; warn if elevated access needed for certain metrics.
- Audit dependencies for vulnerabilities.

### 5.4 Reliability
- Handle system load spikes without crashing.
- Unit/integration tests for metric accuracy and UI rendering.

## 6. Assumptions and Dependencies

- **Assumptions**:
  - Users have Rust installed for building from source.
  - System APIs for metrics are accessible (e.g., no container restrictions).
- **Dependencies**:
  - Ratatui v0.25+.
  - Sysinfo crate.
  - External: None for core functionality; optional for extensions.

## 7. Milestones and Roadmap

### 7.1 MVP (Minimum Viable Product)
- Status: Complete.
- Delivered: core dashboard, CPU and memory charts, built-in theming, config loading, and responsive layout.

### 7.2 Version 1.0
- Status: In progress.
- Remaining focus: packaging, documentation polish, deeper process controls, and platform validation.

### 7.3 Future Releases
- Community-driven features based on GitHub issues.
- Performance optimizations and bug fixes.

## 8. Risks and Mitigations
- **Risk**: Cross-platform inconsistencies in metric collection.
  - **Mitigation**: Use abstraction layers and fallback displays.
- **Risk**: UI complexity overwhelming for simple users.
  - **Mitigation**: Offer a "minimal" mode via config.
- **Risk**: Dependency updates breaking builds.
  - **Mitigation**: Pin versions and use CI/CD for testing.

This PRD serves as a living document and can be iterated upon as the project evolves. For implementation details, refer to the project's GitHub repository (assuming one will be created).
