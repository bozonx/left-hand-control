//! Which profile runs now and the mapper configuration built from it.

use super::{ConfigDocument, ConfigError};
use crate::config::app_match;
use crate::config::auto_switch::{self, AutoSwitchContext};
use crate::config::diagnostics::{self, RuleIssue};
use crate::config::model::{AppSettings, ProfileMode, ProfilePreset, ResolvedAppConfig};

/// Mapper input computed from the document for the current system state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Typed input for the mapper lifecycle API.
    pub mapper: crate::mapper::config::MapperConfig,
    /// Menu contents captured with the mapper configuration.
    pub profile: ProfilePreset,
    /// Profile the rules come from; `None` means native passthrough.
    pub profile_id: Option<String>,
    /// Whether the rules or the auto-mode choice look at window titles,
    /// see [`crate::active_window::set_titles_needed`].
    pub uses_titles: bool,
}

impl RuntimeConfig {
    /// Combine `settings` with the profile chosen to run, refusing blocking
    /// rule problems and dropping disabled or draft rules.
    fn build(
        settings: &AppSettings,
        profile: ProfilePreset,
        profile_id: Option<String>,
    ) -> Result<Self, ConfigError> {
        let mut config =
            ResolvedAppConfig::from_parts(settings.clone(), profile.clone(), profile_id.as_deref());
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
            profile,
            profile_id,
            uses_titles,
        })
    }
}

impl ConfigDocument {
    /// Profile whose rules should run now: the manual choice, or in auto
    /// mode the first matching rule's profile or the default.
    pub fn active_profile_id(
        &self,
        ctx: &AutoSwitchContext,
    ) -> Result<Option<String>, ConfigError> {
        Ok(self.profile_choice(ctx)?.profile_id)
    }

    /// Selected profile and matching rule from one evaluation of the context.
    pub fn profile_choice(
        &self,
        ctx: &AutoSwitchContext,
    ) -> Result<auto_switch::AutoChoice, ConfigError> {
        match self.settings.profile_mode {
            ProfileMode::Manual => Ok(auto_switch::AutoChoice {
                profile_id: self.settings.manual_profile_id.clone(),
                matched_rule_index: None,
            }),
            ProfileMode::Auto => self.auto_choice(ctx),
        }
    }

    /// Which automatic rule applies under `ctx`, whatever the current mode.
    pub fn auto_choice(
        &self,
        ctx: &AutoSwitchContext,
    ) -> Result<auto_switch::AutoChoice, ConfigError> {
        Ok(auto_switch::choose(
            &self.profile_ids()?,
            &self.settings,
            ctx,
        ))
    }

    /// Mapper configuration for the current system state: picks the active profile,
    /// refuses blocking rule problems and drops disabled or draft rules.
    pub fn runtime_config(&self, ctx: &AutoSwitchContext) -> Result<RuntimeConfig, ConfigError> {
        let profile_id = self.active_profile_id(ctx)?;
        self.runtime_config_for_profile(profile_id)
    }

    /// Build an already selected profile without evaluating the context again.
    pub fn runtime_config_for_profile(
        &self,
        profile_id: Option<String>,
    ) -> Result<RuntimeConfig, ConfigError> {
        let profile = self.profile_for_activation(profile_id.as_deref())?;
        RuntimeConfig::build(&self.settings, profile, profile_id)
    }

    pub fn active_profile(&self, ctx: &AutoSwitchContext) -> Result<ProfilePreset, ConfigError> {
        self.profile_for_activation(self.active_profile_id(ctx)?.as_deref())
    }

    /// Rules of profile `id` as they would run. The profile being edited runs
    /// its working copy, unsaved changes included, so edits apply live. A
    /// `None` id means passthrough, except in manual mode where an unnamed
    /// working copy is the active profile itself.
    pub fn profile_for_activation(&self, id: Option<&str>) -> Result<ProfilePreset, ConfigError> {
        if id == self.settings.editing_profile_id.as_deref()
            && (id.is_some() || self.settings.profile_mode == ProfileMode::Manual)
        {
            return Ok(self.profile.clone());
        }
        match id {
            Some(id) => self.load_profile(id),
            None => Ok(ProfilePreset::default()),
        }
    }
}
