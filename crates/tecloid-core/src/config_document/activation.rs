//! Which keymap runs now and the mapper configuration built from it.

use super::{ConfigDocument, ConfigError};
use crate::config::app_match;
use crate::config::auto_switch::{self, AutoSwitchContext};
use crate::config::diagnostics::{self, RuleIssue};
use crate::config::model::{AppSettings, KeymapMode, KeymapPreset, ResolvedAppConfig};

/// Mapper input computed from the document for the current system state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Typed input for the mapper lifecycle API.
    pub mapper: crate::mapper::config::MapperConfig,
    /// Menu contents captured with the mapper configuration.
    pub keymap: KeymapPreset,
    /// Keymap the rules come from; `None` means native passthrough.
    pub keymap_id: Option<String>,
    /// Whether the rules or the auto-mode choice look at window titles,
    /// see [`crate::active_window::set_titles_needed`].
    pub uses_titles: bool,
}

impl RuntimeConfig {
    /// Combine `settings` with the keymap chosen to run, refusing blocking
    /// rule problems and dropping disabled or draft rules.
    fn build(
        settings: &AppSettings,
        keymap: KeymapPreset,
        keymap_id: Option<String>,
    ) -> Result<Self, ConfigError> {
        let mut config =
            ResolvedAppConfig::from_parts(settings.clone(), keymap.clone(), keymap_id.as_deref());
        let blocking: Vec<RuleIssue> = diagnostics::analyze(&config)
            .into_iter()
            .filter(|issue| issue.code.is_error())
            .collect();
        if !blocking.is_empty() {
            return Err(ConfigError::Rules(blocking));
        }
        diagnostics::retain_runtime_rules(&mut config);
        let rule_patterns = config.rules.iter().flat_map(|rule| {
            [
                &rule.condition_window_patterns,
                &rule.condition_excluded_window_patterns,
            ]
            .into_iter()
            .flatten()
            .flatten()
        });
        let auto_patterns = auto_switch::window_patterns(settings);
        let uses_titles = app_match::uses_title(rule_patterns.chain(auto_patterns));
        let mapper =
            crate::mapper::config::MapperConfig::try_from(&config).map_err(ConfigError::Invalid)?;
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        crate::mapper::validation::validate_config(&mapper).map_err(ConfigError::Invalid)?;
        Ok(Self {
            mapper,
            keymap,
            keymap_id,
            uses_titles,
        })
    }
}

impl ConfigDocument {
    /// Keymap whose rules should run now: the manual choice, or in auto
    /// mode the first matching rule's keymap or the default.
    pub fn active_keymap_id(&self, ctx: &AutoSwitchContext) -> Result<Option<String>, ConfigError> {
        Ok(self.keymap_choice(ctx)?.keymap_id)
    }

    /// Selected keymap and matching rule from one evaluation of the context.
    pub fn keymap_choice(
        &self,
        ctx: &AutoSwitchContext,
    ) -> Result<auto_switch::AutoChoice, ConfigError> {
        match self.settings.keymap_mode {
            KeymapMode::Manual => Ok(auto_switch::AutoChoice {
                keymap_id: self.settings.manual_keymap_id.clone(),
                matched_rule_index: None,
            }),
            KeymapMode::Auto => self.auto_choice(ctx),
        }
    }

    /// Which automatic rule applies under `ctx`, whatever the current mode.
    pub fn auto_choice(
        &self,
        ctx: &AutoSwitchContext,
    ) -> Result<auto_switch::AutoChoice, ConfigError> {
        Ok(auto_switch::choose(
            &self.keymap_ids()?,
            &self.settings,
            ctx,
        ))
    }

    /// Mapper configuration for the current system state: picks the active keymap,
    /// refuses blocking rule problems and drops disabled or draft rules.
    pub fn runtime_config(&self, ctx: &AutoSwitchContext) -> Result<RuntimeConfig, ConfigError> {
        let keymap_id = self.active_keymap_id(ctx)?;
        self.runtime_config_for_keymap(keymap_id)
    }

    /// Build an already selected keymap without evaluating the context again.
    pub fn runtime_config_for_keymap(
        &self,
        keymap_id: Option<String>,
    ) -> Result<RuntimeConfig, ConfigError> {
        let keymap = self.keymap_for_activation(keymap_id.as_deref())?;
        RuntimeConfig::build(&self.settings, keymap, keymap_id)
    }

    pub fn active_keymap(&self, ctx: &AutoSwitchContext) -> Result<KeymapPreset, ConfigError> {
        self.keymap_for_activation(self.active_keymap_id(ctx)?.as_deref())
    }

    /// Rules of keymap `id` as they would run. The keymap being edited runs
    /// its working copy, unsaved changes included, so edits apply live. A
    /// `None` id means passthrough, except in manual mode where an unnamed
    /// working copy is the active keymap itself.
    pub fn keymap_for_activation(&self, id: Option<&str>) -> Result<KeymapPreset, ConfigError> {
        if id == self.settings.editing_keymap_id.as_deref()
            && (id.is_some() || self.settings.keymap_mode == KeymapMode::Manual)
        {
            return Ok(self.keymap.clone());
        }
        match id {
            Some(id) => self.load_keymap(id),
            None => Ok(KeymapPreset::default()),
        }
    }
}
