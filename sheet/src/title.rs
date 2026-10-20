//! The title block's twelve cells, as the drawing's record of itself: the time and the date, which sheet
//! and which project, the power and the battery, the profile, the processor and the memory, the network,
//! the temperatures and the fan, how many processes, and who is drawing.
//!
//! The compositor measures the block and every cell in whole physical pixels (compositor/src/title.rs)
//! and publishes them; this works out none of it, only what each cell says and where in the cell it goes.
//! The lettering follows qml/TitleBlock.qml, which it replaces: the label 2 mm in and 1.2 mm down from
//! the cell's corner, the reading 2 mm in and sitting 1.6 mm above the cell's foot.

use serde_json::Value;

/// What one cell says, as this sheet can letter it.
pub struct Cell {
    pub label: String,
    pub value: String,
    /// read beyond what the theme calls alarming: lettered in the alert ink
    pub alert: bool,
    /// the clock, and only the clock: lettered large
    pub large: bool,
}

/// The twelve cells, in the order the compositor measured them, from the one account of what the sheet
/// says (`draftpaper::content`, docs/sheet-content.md). Nothing is decided here: the document carries
/// each reading with its glyphs in `text` and without them in `words`, and this picks between them,
/// which only a renderer can do — it is the only one that knows which faces are installed.
pub fn cells(theme: &crate::sheet::Theme, face: &mut crate::font::Face, workspace: u64, now: std::time::SystemTime, r: &Value, cpu: Option<f64>) -> Vec<Cell> {
    draftpaper::content::title(&theme.raw, workspace, now, r, cpu)
        .iter()
        .map(|c| Cell {
            label: c["label"].as_str().unwrap_or("").to_string(),
            value: crate::glyph::pick(face, theme.glyphs, c),
            alert: c["ink"] == "alert",
            large: c["large"] == true,
        })
        .collect()
}


/// Which cell of the title block is at `x`, `y` in its surface (its own logical pixels, from the block's
/// corner), and whether a click on it does anything yet. The schedules and the graph are the shell's
/// until stage 6; the profile is this client's now.
pub fn under(block: &crate::sheet::Block, x: f64, y: f64) -> Option<(usize, bool)> {
    let i = block.cells.iter().position(|c| {
        let (cx, cy) = (c.0 - block.rect.0, c.1 - block.rect.1);
        x >= cx && y >= cy && x < cx + c.2 && y < cy + c.3
    })?;
    // the profile's cell: the sixth, counting from the clock
    Some((i, i == 6))
}

/// The next power profile after this one, as the title block's cell cycles them.
pub fn next_profile(now: &str) -> &'static str {
    const ORDER: [&str; 3] = ["power-saver", "balanced", "performance"];
    let i = ORDER.iter().position(|p| *p == now).unwrap_or(ORDER.len() - 1);
    ORDER[(i + 1) % ORDER.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_cell_cycles_through_them() {
        assert_eq!(next_profile("power-saver"), "balanced");
        assert_eq!(next_profile("balanced"), "performance");
        assert_eq!(next_profile("performance"), "power-saver");
        // nothing read yet, or a profile this machine does not have: start at the beginning
        assert_eq!(next_profile(""), "power-saver");
    }

    #[test]
    fn a_click_finds_the_cell_it_is_in() {
        let block = crate::sheet::Block { rect: (100.0, 200.0, 300.0, 100.0), cells: vec![(100.0, 200.0, 150.0, 50.0), (250.0, 200.0, 150.0, 50.0)] };
        assert_eq!(under(&block, 10.0, 10.0).map(|(i, _)| i), Some(0));
        assert_eq!(under(&block, 160.0, 10.0).map(|(i, _)| i), Some(1));
        // past the block: nothing
        assert_eq!(under(&block, 400.0, 10.0), None);
    }

    // what each cell says is the document's and is tested with it (cli/src/content.rs). What is left
    // here is the one thing only a renderer can settle: whether this sheet can letter the glyphs.
}
