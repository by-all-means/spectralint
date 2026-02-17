mod agent_guidelines;
mod credential_exposure;
mod custom_pattern;
mod dangerous_command;
mod dead_reference;
mod emoji_density;
mod enum_drift;
mod file_size;
mod heading_hierarchy;
mod macros;
mod missing_essential_sections;
mod missing_verification;
mod naming_inconsistency;
mod negative_only_framing;
mod placeholder_text;
mod prompt_injection_vector;
mod session_journal;
mod stale_reference;
pub(crate) mod utils;
mod vague_directive;

use crate::config::Config;
use crate::engine::cross_ref::CheckerContext;
use crate::types::CheckResult;

pub(crate) trait Checker: Send + Sync {
    fn check(&self, ctx: &CheckerContext) -> CheckResult;
}

pub(crate) fn all_checkers(config: &Config) -> Vec<Box<dyn Checker>> {
    let mut checkers: Vec<Box<dyn Checker>> = Vec::new();

    if config.checkers.dead_reference.enabled {
        checkers.push(Box::new(dead_reference::DeadReferenceChecker));
    }
    if config.checkers.vague_directive.enabled {
        checkers.push(Box::new(vague_directive::VagueDirectiveChecker::new(
            config.checkers.vague_directive.strict,
            &config.checkers.vague_directive.extra_patterns,
            &config.checkers.vague_directive.scope,
        )));
    }
    if config.checkers.naming_inconsistency.enabled {
        checkers.push(Box::new(
            naming_inconsistency::NamingInconsistencyChecker::new(
                &config.checkers.naming_inconsistency.scope,
            ),
        ));
    }
    if config.strict || config.checkers.enum_drift.enabled {
        checkers.push(Box::new(enum_drift::EnumDriftChecker::new(
            &config.checkers.enum_drift.scope,
        )));
    }
    if config.strict || config.checkers.agent_guidelines.enabled {
        checkers.push(Box::new(agent_guidelines::AgentGuidelinesChecker::new(
            &config.checkers.agent_guidelines.scope,
        )));
    }
    if config.checkers.placeholder_text.enabled {
        checkers.push(Box::new(placeholder_text::PlaceholderTextChecker::new(
            &config.checkers.placeholder_text.scope,
        )));
    }
    if config.checkers.file_size.enabled {
        checkers.push(Box::new(file_size::FileSizeChecker::new(
            &config.checkers.file_size,
        )));
    }
    if config.checkers.credential_exposure.enabled {
        checkers.push(Box::new(
            credential_exposure::CredentialExposureChecker::new(
                &config.checkers.credential_exposure.scope,
            ),
        ));
    }
    if config.strict || config.checkers.heading_hierarchy.enabled {
        checkers.push(Box::new(heading_hierarchy::HeadingHierarchyChecker::new(
            &config.checkers.heading_hierarchy.scope,
        )));
    }
    if config.checkers.dangerous_command.enabled {
        checkers.push(Box::new(dangerous_command::DangerousCommandChecker::new(
            &config.checkers.dangerous_command.scope,
        )));
    }
    if config.checkers.stale_reference.enabled {
        checkers.push(Box::new(stale_reference::StaleReferenceChecker::new(
            &config.checkers.stale_reference.scope,
        )));
    }
    if config.strict || config.checkers.emoji_density.enabled {
        checkers.push(Box::new(emoji_density::EmojiDensityChecker::new(
            &config.checkers.emoji_density,
        )));
    }
    if config.checkers.session_journal.enabled {
        checkers.push(Box::new(session_journal::SessionJournalChecker::new(
            &config.checkers.session_journal.scope,
        )));
    }
    if config.checkers.missing_essential_sections.enabled {
        checkers.push(Box::new(
            missing_essential_sections::MissingEssentialSectionsChecker::new(
                &config.checkers.missing_essential_sections,
            ),
        ));
    }
    if config.checkers.prompt_injection_vector.enabled {
        checkers.push(Box::new(
            prompt_injection_vector::PromptInjectionVectorChecker::new(
                &config.checkers.prompt_injection_vector.scope,
            ),
        ));
    }
    if config.strict || config.checkers.missing_verification.enabled {
        checkers.push(Box::new(
            missing_verification::MissingVerificationChecker::new(
                &config.checkers.missing_verification,
            ),
        ));
    }
    if config.strict || config.checkers.negative_only_framing.enabled {
        checkers.push(Box::new(
            negative_only_framing::NegativeOnlyFramingChecker::new(
                &config.checkers.negative_only_framing,
            ),
        ));
    }
    if !config.checkers.custom_patterns.is_empty() {
        checkers.push(Box::new(custom_pattern::CustomPatternChecker::new(
            &config.checkers.custom_patterns,
        )));
    }

    checkers
}
