use zellij_tile::prelude::PaneManifest;

/// Returns the terminal pane id the plugin should read from.
///
/// Four-tier preference (see doc/architecture.md):
///   1. Currently focused non-plugin pane
///   2. `hint` — last_focused_non_plugin recorded in background PaneUpdates
///   3. First tiled, non-suppressed non-plugin pane
///   4. Any non-plugin pane
pub fn pick(manifest: &PaneManifest, hint: Option<u32>, active_tab: Option<usize>) -> Option<u32> {
    let mut focused_non_plugin: Option<u32> = None;
    let mut hint_exists = false;
    let mut first_tiled: Option<u32> = None;
    let mut first_any: Option<u32> = None;

    let tab_panes: Box<dyn Iterator<Item = &Vec<zellij_tile::prelude::PaneInfo>>> =
        match active_tab {
            Some(idx) => match manifest.panes.get(&idx) {
                Some(panes) => Box::new(std::iter::once(panes)),
                None => Box::new(manifest.panes.values()),
            },
            None => Box::new(manifest.panes.values()),
        };

    for panes in tab_panes {
        for pane in panes {
            if pane.is_plugin { continue; }
            if pane.is_focused { focused_non_plugin = Some(pane.id); }
            if Some(pane.id) == hint { hint_exists = true; }
            if first_any.is_none() { first_any = Some(pane.id); }
            if first_tiled.is_none() && !pane.is_floating && !pane.is_suppressed {
                first_tiled = Some(pane.id);
            }
        }
    }

    focused_non_plugin
        .or(if hint_exists { hint } else { None })
        .or(first_tiled)
        .or(first_any)
}
