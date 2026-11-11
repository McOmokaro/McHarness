//! Bento Grid geometry: a 2×2 pane grid over a 3-line command bar.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// The five regions of the command center.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BentoLayout {
    /// Top-left: The Brain.
    pub brain: Rect,
    /// Top-right: The Key Farm.
    pub keyfarm: Rect,
    /// Bottom-left: The Arsenal.
    pub arsenal: Rect,
    /// Bottom-right: The Overnight Agent.
    pub overnight: Rect,
    /// Bottom command bar.
    pub command: Rect,
}

/// Compute the bento grid for a terminal `area`.
pub fn bento_layout(area: Rect) -> BentoLayout {
    // Body (pane grid) on top, command bar (3 lines) at the bottom.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(3)])
        .split(area);
    let body = rows[0];
    let command = rows[1];

    // Split the body into two rows, then each row into two columns.
    let v = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(body);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(v[0]);
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(v[1]);

    BentoLayout {
        brain: top[0],
        keyfarm: top[1],
        arsenal: bottom[0],
        overnight: bottom[1],
        command,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panes_fit_and_do_not_overlap() {
        let area = Rect::new(0, 0, 120, 40);
        let l = bento_layout(area);

        let panes = [l.brain, l.keyfarm, l.arsenal, l.overnight];
        for p in panes {
            assert!(p.right() <= area.right(), "pane overflows right");
            assert!(p.bottom() <= area.bottom(), "pane overflows bottom");
        }
        // Command bar sits below every pane.
        for p in panes {
            assert!(p.bottom() <= l.command.top());
        }
        // 2×2 grid: left panes left of right panes; top panes above bottom.
        assert!(l.brain.right() <= l.keyfarm.left());
        assert!(l.arsenal.right() <= l.overnight.left());
        assert!(l.brain.bottom() <= l.arsenal.top());
    }

    #[test]
    fn command_bar_is_three_lines() {
        let area = Rect::new(0, 0, 100, 30);
        let l = bento_layout(area);
        assert_eq!(l.command.height, 3);
    }
}
