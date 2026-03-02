mod app;
mod config;
mod metrics;
mod theme;
mod ui;

use std::io;

use app::App;
use config::load;
use ratatui::DefaultTerminal;

fn main() -> io::Result<()> {
    let loaded = load();
    for warning in &loaded.warnings {
        eprintln!("config: {warning}");
    }

    let terminal = ratatui::init();
    let result = run(terminal, loaded.config);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal, config: config::AppConfig) -> io::Result<()> {
    let mut app = App::new(config);

    while !app.should_quit() {
        app.refresh_if_due();
        terminal.draw(|frame| ui::render(frame, &app))?;
        app.handle_events()?;
    }

    Ok(())
}
