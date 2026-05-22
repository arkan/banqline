// Full TUI application with ratatui.
// Layout: [global tabs] then [sidebar accounts | detail area with sub-tabs].
pub mod app;
pub mod views;
pub mod widgets;

use anyhow::Result;

use crate::config::Config;

/// Launch the interactive TUI.
pub async fn run(cfg: Config) -> Result<()> {
    app::App::run(cfg).await
}
