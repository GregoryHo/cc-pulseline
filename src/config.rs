#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlyphMode {
    Ascii,
    Icon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorTheme {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidthDegradeStrategy {
    DropActivityLinesFirst,
    CompressLine2,
    CompressCoreLines,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenderConfig {
    pub glyph_mode: GlyphMode,
    pub color_enabled: bool,
    pub color_theme: ColorTheme,
    pub max_tool_lines: usize,
    pub max_agent_lines: usize,
    pub transcript_window_events: usize,
    pub transcript_poll_throttle_ms: u64,
    pub terminal_width: Option<usize>,
    pub degrade_order: Vec<WidthDegradeStrategy>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            glyph_mode: GlyphMode::Ascii,
            color_enabled: false,
            color_theme: ColorTheme::Dark,
            max_tool_lines: 2,
            max_agent_lines: 2,
            transcript_window_events: 400,
            transcript_poll_throttle_ms: 250,
            terminal_width: None,
            degrade_order: vec![
                WidthDegradeStrategy::DropActivityLinesFirst,
                WidthDegradeStrategy::CompressLine2,
                WidthDegradeStrategy::CompressCoreLines,
            ],
        }
    }
}
