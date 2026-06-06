//! Thin wrapper over winvd. (Spike: read current desktop, raw.)

use anyhow::{anyhow, Result};

/// Read the current virtual desktop: 0-based index and its (possibly empty) name.
pub fn current_index_and_name() -> Result<(u32, String)> {
    let desktop =
        winvd::get_current_desktop().map_err(|e| anyhow!("get_current_desktop: {e:?}"))?;
    let index = desktop
        .get_index()
        .map_err(|e| anyhow!("get_index: {e:?}"))?;
    let name = desktop.get_name().unwrap_or_default();
    Ok((index, name))
}

use crate::label::format_label;

/// Read the current desktop and return its formatted badge label.
pub fn current_label() -> Result<String> {
    let (index, name) = current_index_and_name()?;
    Ok(format_label(index, &name))
}
