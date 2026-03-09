mod absolute_path;
mod agent_guidelines;
mod ambiguous_scope_reference;
mod bare_url;
mod boilerplate_template;
mod broken_anchor_link;
mod broken_table;
mod circular_reference;
mod click_here_link;
mod command_validation;
mod command_without_codeblock;
mod conflicting_directives;
mod context_window_waste;
mod copied_meta_instructions;
mod credential_exposure;
mod cross_file_contradiction;
mod custom_pattern;
mod dangerous_command;
mod dead_reference;
mod double_negation;
mod duplicate_instruction_file;
mod duplicate_section;
mod emoji_density;
mod emphasis_overuse;
mod empty_code_block;
mod empty_heading;
mod enum_drift;
mod excessive_nesting;
mod file_size;
mod generated_attribution;
mod generic_instruction;
mod hardcoded_file_structure;
mod hardcoded_windows_path;
mod heading_hierarchy;
mod imperative_heading;
mod inconsistent_command_prefix;
mod instruction_density;
mod instruction_without_context;
mod large_code_block;
mod long_paragraph;
mod macros;
mod misordered_steps;
mod missing_essential_sections;
mod missing_examples;
mod missing_role_definition;
mod missing_standard_file;
mod missing_verification;
mod missing_verification_step;
mod naming_inconsistency;
mod negative_only_framing;
mod orphaned_section;
mod outdated_model_reference;
mod placeholder_text;
mod placeholder_url;
mod prompt_injection_vector;
mod redundant_directive;
mod repeated_word;
mod section_length_imbalance;
mod session_journal;
mod stale_file_tree;
mod stale_reference;
mod stale_style_rule;
mod token_budget;
mod unbounded_scope;
mod unclosed_fence;
mod undocumented_env_var;
mod untagged_code_block;
mod unversioned_stack_reference;
pub(crate) mod utils;
mod vague_directive;
mod xml_document_wrapper;

use crate::config::Config;
use crate::engine::cross_ref::CheckerContext;
use crate::types::{CheckResult, RuleMeta};

pub(crate) trait Checker: Send + Sync {
    /// Static metadata about this checker (name, description, severity, strict_only).
    #[allow(dead_code)]
    fn meta(&self) -> RuleMeta;

    fn check(&self, ctx: &CheckerContext) -> CheckResult;
}

/// Collect metadata from all checkers (using default config to instantiate them).
/// Useful for generating rule lists and validation.
#[allow(dead_code)]
pub(crate) fn all_checker_meta() -> Vec<RuleMeta> {
    let config = Config::default_with_all_enabled();
    all_checkers(&config).iter().map(|c| c.meta()).collect()
}

pub(crate) fn all_checkers(config: &Config) -> Vec<Box<dyn Checker>> {
    let mut checkers: Vec<Box<dyn Checker>> = Vec::new();

    if config.checkers.dead_reference.enabled {
        checkers.push(Box::new(dead_reference::DeadReferenceChecker));
    }
    if config.strict || config.checkers.vague_directive.enabled {
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
            config.strict,
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
    if config.strict || config.checkers.missing_essential_sections.enabled {
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
    if config.checkers.conflicting_directives.enabled {
        checkers.push(Box::new(
            conflicting_directives::ConflictingDirectivesChecker::new(
                &config.checkers.conflicting_directives.scope,
            ),
        ));
    }
    if config.strict || config.checkers.missing_role_definition.enabled {
        checkers.push(Box::new(
            missing_role_definition::MissingRoleDefinitionChecker::new(
                &config.checkers.missing_role_definition.scope,
            ),
        ));
    }
    if config.strict || config.checkers.redundant_directive.enabled {
        checkers.push(Box::new(
            redundant_directive::RedundantDirectiveChecker::new(
                &config.checkers.redundant_directive,
            ),
        ));
    }
    if config.strict || config.checkers.instruction_density.enabled {
        checkers.push(Box::new(
            instruction_density::InstructionDensityChecker::new(
                &config.checkers.instruction_density,
            ),
        ));
    }
    if config.strict || config.checkers.missing_examples.enabled {
        checkers.push(Box::new(missing_examples::MissingExamplesChecker::new(
            &config.checkers.missing_examples.scope,
        )));
    }
    if config.strict || config.checkers.unbounded_scope.enabled {
        checkers.push(Box::new(unbounded_scope::UnboundedScopeChecker::new(
            &config.checkers.unbounded_scope.scope,
        )));
    }
    if config.checkers.circular_reference.enabled {
        checkers.push(Box::new(circular_reference::CircularReferenceChecker::new(
            &config.checkers.circular_reference.scope,
        )));
    }
    if config.checkers.large_code_block.enabled {
        checkers.push(Box::new(large_code_block::LargeCodeBlockChecker::new(
            &config.checkers.large_code_block,
        )));
    }
    if config.checkers.duplicate_section.enabled {
        checkers.push(Box::new(duplicate_section::DuplicateSectionChecker::new(
            &config.checkers.duplicate_section.scope,
        )));
    }
    if config.checkers.absolute_path.enabled {
        checkers.push(Box::new(absolute_path::AbsolutePathChecker::new(
            &config.checkers.absolute_path.scope,
        )));
    }
    if config.strict || config.checkers.generic_instruction.enabled {
        checkers.push(Box::new(
            generic_instruction::GenericInstructionChecker::new(
                &config.checkers.generic_instruction.scope,
            ),
        ));
    }
    if config.checkers.misordered_steps.enabled {
        checkers.push(Box::new(misordered_steps::MisorderedStepsChecker::new(
            &config.checkers.misordered_steps.scope,
        )));
    }
    if config.strict || config.checkers.section_length_imbalance.enabled {
        checkers.push(Box::new(
            section_length_imbalance::SectionLengthImbalanceChecker::new(
                &config.checkers.section_length_imbalance,
            ),
        ));
    }
    if config.checkers.unclosed_fence.enabled {
        checkers.push(Box::new(unclosed_fence::UnclosedFenceChecker::new(
            &config.checkers.unclosed_fence.scope,
        )));
    }
    if config.strict || config.checkers.untagged_code_block.enabled {
        checkers.push(Box::new(
            untagged_code_block::UntaggedCodeBlockChecker::new(
                &config.checkers.untagged_code_block.scope,
            ),
        ));
    }
    if config.checkers.duplicate_instruction_file.enabled {
        checkers.push(Box::new(
            duplicate_instruction_file::DuplicateInstructionFileChecker::new(
                &config.checkers.duplicate_instruction_file.scope,
            ),
        ));
    }
    if config.checkers.outdated_model_reference.enabled {
        checkers.push(Box::new(
            outdated_model_reference::OutdatedModelReferenceChecker::new(
                &config.checkers.outdated_model_reference.scope,
            ),
        ));
    }
    if config.checkers.broken_anchor_link.enabled {
        checkers.push(Box::new(broken_anchor_link::BrokenAnchorLinkChecker::new(
            &config.checkers.broken_anchor_link.scope,
        )));
    }
    if config.checkers.broken_table.enabled {
        checkers.push(Box::new(broken_table::BrokenTableChecker::new(
            &config.checkers.broken_table.scope,
        )));
    }
    if config.checkers.placeholder_url.enabled {
        checkers.push(Box::new(placeholder_url::PlaceholderUrlChecker::new(
            &config.checkers.placeholder_url.scope,
        )));
    }
    if config.strict || config.checkers.emphasis_overuse.enabled {
        checkers.push(Box::new(emphasis_overuse::EmphasisOveruseChecker::new(
            &config.checkers.emphasis_overuse,
        )));
    }
    if config.checkers.boilerplate_template.enabled {
        checkers.push(Box::new(
            boilerplate_template::BoilerplateTemplateChecker::new(
                &config.checkers.boilerplate_template.scope,
            ),
        ));
    }
    if config.checkers.generated_attribution.enabled {
        checkers.push(Box::new(
            generated_attribution::GeneratedAttributionChecker::new(
                &config.checkers.generated_attribution.scope,
            ),
        ));
    }
    if config.checkers.orphaned_section.enabled {
        checkers.push(Box::new(orphaned_section::OrphanedSectionChecker::new(
            &config.checkers.orphaned_section.scope,
        )));
    }
    if config.strict || config.checkers.excessive_nesting.enabled {
        checkers.push(Box::new(excessive_nesting::ExcessiveNestingChecker::new(
            &config.checkers.excessive_nesting,
        )));
    }
    if config.strict || config.checkers.cross_file_contradiction.enabled {
        checkers.push(Box::new(
            cross_file_contradiction::CrossFileContradictionChecker::new(
                &config.checkers.cross_file_contradiction.scope,
            ),
        ));
    }
    if config.strict || config.checkers.instruction_without_context.enabled {
        checkers.push(Box::new(
            instruction_without_context::InstructionWithoutContextChecker::new(
                &config.checkers.instruction_without_context.scope,
            ),
        ));
    }
    if config.checkers.ambiguous_scope_reference.enabled {
        checkers.push(Box::new(
            ambiguous_scope_reference::AmbiguousScopeReferenceChecker::new(
                &config.checkers.ambiguous_scope_reference.scope,
            ),
        ));
    }
    if config.checkers.context_window_waste.enabled {
        checkers.push(Box::new(
            context_window_waste::ContextWindowWasteChecker::new(
                &config.checkers.context_window_waste.scope,
            ),
        ));
    }
    if config.checkers.stale_style_rule.enabled {
        checkers.push(Box::new(stale_style_rule::StaleStyleRuleChecker::new(
            &config.checkers.stale_style_rule.scope,
        )));
    }
    if config.checkers.hardcoded_windows_path.enabled {
        checkers.push(Box::new(
            hardcoded_windows_path::HardcodedWindowsPathChecker::new(
                &config.checkers.hardcoded_windows_path.scope,
            ),
        ));
    }
    if config.checkers.hardcoded_file_structure.enabled {
        checkers.push(Box::new(
            hardcoded_file_structure::HardcodedFileStructureChecker::new(
                &config.checkers.hardcoded_file_structure.scope,
            ),
        ));
    }
    if config.checkers.stale_file_tree.enabled {
        checkers.push(Box::new(stale_file_tree::StaleFileTreeChecker::new(
            &config.checkers.stale_file_tree.scope,
        )));
    }
    if config.checkers.command_validation.enabled {
        checkers.push(Box::new(command_validation::CommandValidationChecker::new(
            &config.checkers.command_validation.scope,
        )));
    }
    if config.checkers.token_budget.enabled {
        checkers.push(Box::new(token_budget::TokenBudgetChecker::new(
            &config.checkers.token_budget,
        )));
    }
    if config.strict || config.checkers.unversioned_stack_reference.enabled {
        checkers.push(Box::new(
            unversioned_stack_reference::UnversionedStackReferenceChecker::new(
                &config.checkers.unversioned_stack_reference.scope,
            ),
        ));
    }
    if config.strict || config.checkers.missing_standard_file.enabled {
        checkers.push(Box::new(missing_standard_file::MissingStandardFileChecker));
    }
    if config.strict || config.checkers.bare_url.enabled {
        checkers.push(Box::new(bare_url::BareUrlChecker::new(
            &config.checkers.bare_url.scope,
        )));
    }
    if config.strict || config.checkers.repeated_word.enabled {
        checkers.push(Box::new(repeated_word::RepeatedWordChecker::new(
            &config.checkers.repeated_word.scope,
        )));
    }
    if config.strict || config.checkers.undocumented_env_var.enabled {
        checkers.push(Box::new(
            undocumented_env_var::UndocumentedEnvVarChecker::new(
                &config.checkers.undocumented_env_var.scope,
            ),
        ));
    }
    if config.strict || config.checkers.empty_code_block.enabled {
        checkers.push(Box::new(empty_code_block::EmptyCodeBlockChecker::new(
            &config.checkers.empty_code_block.scope,
        )));
    }
    if config.strict || config.checkers.click_here_link.enabled {
        checkers.push(Box::new(click_here_link::ClickHereLinkChecker::new(
            &config.checkers.click_here_link.scope,
        )));
    }
    if config.strict || config.checkers.double_negation.enabled {
        checkers.push(Box::new(double_negation::DoubleNegationChecker::new(
            &config.checkers.double_negation.scope,
        )));
    }
    if config.strict || config.checkers.imperative_heading.enabled {
        checkers.push(Box::new(imperative_heading::ImperativeHeadingChecker::new(
            &config.checkers.imperative_heading.scope,
        )));
    }
    if config.strict || config.checkers.inconsistent_command_prefix.enabled {
        checkers.push(Box::new(
            inconsistent_command_prefix::InconsistentCommandPrefixChecker::new(
                &config.checkers.inconsistent_command_prefix.scope,
            ),
        ));
    }
    if config.strict || config.checkers.empty_heading.enabled {
        checkers.push(Box::new(empty_heading::EmptyHeadingChecker::new(
            &config.checkers.empty_heading.scope,
        )));
    }
    if config.strict || config.checkers.copied_meta_instructions.enabled {
        checkers.push(Box::new(
            copied_meta_instructions::CopiedMetaInstructionsChecker::new(
                &config.checkers.copied_meta_instructions.scope,
            ),
        ));
    }
    if config.strict || config.checkers.long_paragraph.enabled {
        checkers.push(Box::new(long_paragraph::LongParagraphChecker::new(
            &config.checkers.long_paragraph,
        )));
    }
    if config.strict || config.checkers.command_without_codeblock.enabled {
        checkers.push(Box::new(
            command_without_codeblock::CommandWithoutCodeblockChecker::new(
                &config.checkers.command_without_codeblock.scope,
            ),
        ));
    }
    if config.strict || config.checkers.missing_verification_step.enabled {
        checkers.push(Box::new(
            missing_verification_step::MissingVerificationStepChecker::new(
                &config.checkers.missing_verification_step.scope,
            ),
        ));
    }
    if config.strict || config.checkers.xml_document_wrapper.enabled {
        checkers.push(Box::new(
            xml_document_wrapper::XmlDocumentWrapperChecker::new(
                &config.checkers.xml_document_wrapper.scope,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_checkers_have_unique_meta_names() {
        let metas = all_checker_meta();
        assert!(!metas.is_empty(), "all_checker_meta() returned no checkers");

        let mut seen = HashSet::new();
        for meta in &metas {
            assert!(
                !meta.name.is_empty(),
                "Checker meta has empty name: {:?}",
                meta
            );
            assert!(
                !meta.description.is_empty(),
                "Checker '{}' has empty description",
                meta.name
            );
            assert!(
                seen.insert(meta.name),
                "Duplicate checker name: '{}'",
                meta.name
            );
        }
    }

    #[test]
    fn meta_count_matches_checker_count() {
        let config = Config::default_with_all_enabled();
        let checkers = all_checkers(&config);
        let metas = all_checker_meta();
        assert_eq!(
            checkers.len(),
            metas.len(),
            "all_checkers() and all_checker_meta() return different counts"
        );
    }

    /// Rules that appear in AVAILABLE_RULES but are not always instantiated as
    /// standalone checkers: pseudo-rules from the suppression system, and the
    /// `custom` rule which requires user-defined patterns in config.
    const NON_DEFAULT_RULES: &[&str] = &["unused-suppression", "invalid-suppression", "custom"];

    #[test]
    fn meta_names_match_available_rules() {
        use crate::cli::explain::AVAILABLE_RULES;

        let metas = all_checker_meta();
        let meta_names: HashSet<&str> = metas.iter().map(|m| m.name).collect();
        let rule_names: HashSet<&str> = AVAILABLE_RULES
            .iter()
            .map(|(name, _)| *name)
            .filter(|n| !NON_DEFAULT_RULES.contains(n))
            .collect();

        let in_meta_not_rules: Vec<_> = meta_names.difference(&rule_names).collect();
        let in_rules_not_meta: Vec<_> = rule_names.difference(&meta_names).collect();

        assert!(
            in_meta_not_rules.is_empty() && in_rules_not_meta.is_empty(),
            "Mismatch between checker meta() names and AVAILABLE_RULES:\n  \
             In meta() but not AVAILABLE_RULES: {:?}\n  \
             In AVAILABLE_RULES but not meta(): {:?}",
            in_meta_not_rules,
            in_rules_not_meta,
        );
    }
}
