pub mod agent_guidelines;
pub mod custom_pattern;
pub mod dead_reference;
pub mod enum_drift;
pub mod macros;
pub mod naming_inconsistency;
pub mod utils;
pub mod vague_directive;

use crate::config::Config;
use crate::engine::cross_ref::CheckerContext;
use crate::types::CheckResult;

pub trait Checker: Send + Sync {
    fn check(&self, ctx: &CheckerContext) -> CheckResult;
}

pub fn all_checkers(config: &Config) -> Vec<Box<dyn Checker>> {
    let mut checkers: Vec<Box<dyn Checker>> = Vec::new();

    if config.checkers.dead_reference.enabled {
        checkers.push(Box::new(dead_reference::DeadReferenceChecker));
    }
    if config.checkers.vague_directive.enabled {
        checkers.push(Box::new(vague_directive::VagueDirectiveChecker::new(
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
    if config.checkers.enum_drift.enabled {
        checkers.push(Box::new(enum_drift::EnumDriftChecker::new(
            &config.checkers.enum_drift.scope,
        )));
    }
    if config.checkers.agent_guidelines.enabled {
        checkers.push(Box::new(agent_guidelines::AgentGuidelinesChecker::new(
            &config.checkers.agent_guidelines.scope,
        )));
    }
    if !config.checkers.custom_patterns.is_empty() {
        checkers.push(Box::new(custom_pattern::CustomPatternChecker::new(
            &config.checkers.custom_patterns,
        )));
    }

    checkers
}
