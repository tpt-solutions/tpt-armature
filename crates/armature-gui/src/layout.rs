//! Top-level panel layout for the application shell.
//!
//! The design follows the spec: flex containers composed by hand (no built-in
//! docking). The [`PanelLayout`] enumerates the three primary views and the
//! order in which they are tiled.

/// A primary view surface in the GUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// Raw byte viewer with inline assembly mapping.
    Hex,
    /// Syntax-highlighted assembly with clickable X-refs.
    Assembly,
    /// Control-flow graph node-and-edge canvas.
    Graph,
}

impl Panel {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Panel::Hex => "Hex",
            Panel::Assembly => "Assembly",
            Panel::Graph => "Graph",
        }
    }
}

/// The ordered set of panels composing the application window.
#[derive(Debug, Clone)]
pub struct PanelLayout {
    /// Panels in left-to-right / top-to-bottom tiling order.
    pub panels: Vec<Panel>,
}

impl PanelLayout {
    /// The canonical three-pane layout: Hex | Assembly | Graph.
    pub fn default_triple() -> Self {
        PanelLayout {
            panels: vec![Panel::Hex, Panel::Assembly, Panel::Graph],
        }
    }

    /// Number of panels.
    pub fn len(&self) -> usize {
        self.panels.len()
    }

    /// Whether the layout is empty.
    pub fn is_empty(&self) -> bool {
        self.panels.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_has_three_panels() {
        let layout = PanelLayout::default_triple();
        assert_eq!(layout.len(), 3);
        assert_eq!(layout.panels[0], Panel::Hex);
        assert_eq!(layout.panels[2], Panel::Graph);
        assert_eq!(Panel::Assembly.label(), "Assembly");
    }
}
