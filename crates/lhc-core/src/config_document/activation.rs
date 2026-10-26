//! Which layout runs now and the mapper configuration built from it.

use super::{ConfigDocument, ConfigError};
use crate::profile::app_match;
use crate::profile::auto_switch::{self, AutoSwitchContext};
use crate::profile::diagnostics::{self, RuleIssue};
use crate::profile::model::{AppConfig, AppSettings, LayoutMode, LayoutPreset};

/// Mapper input computed from the document for the current system state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// JSON for [`crate::mapper::runtime::start`] / `update_config`.
    pub json: String,
    /// Layout the rules come from; `None` means native passthrough.
    pub layout_id: Option<String>,
    /// Whether the rules or the auto-mode choice look at window titles,
    /// see [`crate::active_window::set_titles_needed`].
    pub uses_titles: bool,
}

impl RuntimeConfig {
    /// Combine `settings` with the layout chosen to run, refusing blocking
    /// rule problems and dropping disabled or draft rules.
    fn build(
        settings: &AppSettings,
        layout: LayoutPreset,
        layout_id: Option<String>,
    ) -> Result<Self, ConfigError> {
        let mut config = AppConfig::from_parts(settings.clone(), layout, layout_id.as_deref());
        let blocking: Vec<RuleIssue> = diagnostics::analyze(&config)
            .into_iter()
            .filter(|issue| issue.code.is_error())
            .collect();
        if !blocking.is_empty() {
            return Err(ConfigError::Rules(blocking));
        }
        diagnostics::runtime_rules(&mut config);
        let rule_patterns = config.rules.iter().flat_map(|rule| {
            [
                &rule.condition_apps_whitelist,
                &rule.condition_apps_blacklist,
            ]
            .into_iter()
            .flatten()
            .flatten()
        });
        let auto_patterns = settings
            .auto_rules
            .iter()
            .filter(|_| settings.layout_mode == LayoutMode::Auto)
            .flat_map(|rule| {
                rule.conditions
                    .apps
                    .iter()
                    .chain(&rule.conditions.excluded_apps)
            });
        let uses_titles = app_match::uses_title(rule_patterns.chain(auto_patterns));
        let json = config.to_json();
        validate_for_mapper(&json)?;
        Ok(Self {
            json,
            layout_id,
            uses_titles,
        })
    }
}

impl ConfigDocument {
    /// Layout whose rules should run now: the manual choice, or in auto
    /// mode the first matching rule's layout or the default.
    pub fn active_layout_id(&self, ctx: &AutoSwitchContext) -> Result<Option<String>, ConfigError> {
        Ok(match self.settings.layout_mode {
            LayoutMode::Manual => self.settings.manual_active_layout_id.clone(),
            LayoutMode::Auto => self.auto_choice(ctx)?.layout_id,
        })
    }

    /// Which automatic rule applies under `ctx`, whatever the current mode.
    pub fn auto_choice(
        &self,
        ctx: &AutoSwitchContext,
    ) -> Result<auto_switch::AutoChoice, ConfigError> {
        Ok(auto_switch::choose(
            &self.layout_ids()?,
            &self.settings,
            ctx,
        ))
    }

    /// Mapper configuration for the current system state: picks the active layout,
    /// refuses blocking rule problems and drops disabled or draft rules.
    pub fn runtime_config(&self, ctx: &AutoSwitchContext) -> Result<RuntimeConfig, ConfigError> {
        let layout_id = self.active_layout_id(ctx)?;
        // In auto mode "no match" means passthrough, never the current layout.
        let layout = self.layout_for_activation(layout_id.as_deref())?;
        RuntimeConfig::build(&self.settings, layout, layout_id)
    }

    pub fn active_layout(&self, ctx: &AutoSwitchContext) -> Result<LayoutPreset, ConfigError> {
        self.layout_for_activation(self.active_layout_id(ctx)?.as_deref())
    }

    pub fn layout_for_activation(&self, id: Option<&str>) -> Result<LayoutPreset, ConfigError> {
        if id == self.settings.current_layout_id.as_deref()
            && (id.is_some() || self.settings.layout_mode == LayoutMode::Manual)
        {
            return Ok(self.layout.clone());
        }
        match id {
            Some(id) => self.load_layout(id),
            None => Ok(LayoutPreset::default()),
        }
    }
}

/// Run the mapper's own validation, which also checks key names.
fn validate_for_mapper(json: &str) -> Result<(), ConfigError> {
    let config: crate::mapper::config::AppConfig =
        serde_json::from_str(json).map_err(|error| ConfigError::Invalid(error.to_string()))?;
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    crate::mapper::validation::validate_config(&config).map_err(ConfigError::Invalid)?;
    let _ = config;
    Ok(())
}
