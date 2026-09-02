//! Edition-routed source frontends.
//!
//! Edition 2026 remains frozen on the legacy lexer/parser path. Edition 2027
//! deliberately has its own entry point and post-parse constitution checks so
//! new-edition restrictions cannot silently change old source semantics. The
//! initial preview reuses the proven token and expression machinery; grammar
//! additions belong only in `next` and must still lower to the shared AST and
//! typed semantic foundation.

use crate::ast;
use crate::edition::CellScriptEdition;
use crate::error::{CompileError, Result};
use crate::lexer;
use crate::parser;

mod migrate;
mod next;

pub use migrate::{migrate_source_to_2027, MigrationCandidate, MigrationKind};

pub fn parse(source: &str, edition: CellScriptEdition) -> Result<ast::Module> {
    match edition {
        CellScriptEdition::Edition2026 => legacy::parse(source),
        CellScriptEdition::Edition2027 => next::parse(source),
    }
}

pub fn parse_diagnostics(source: &str, edition: CellScriptEdition) -> std::result::Result<ast::Module, Vec<CompileError>> {
    match edition {
        CellScriptEdition::Edition2026 => legacy::parse_diagnostics(source),
        CellScriptEdition::Edition2027 => next::parse_diagnostics(source),
    }
}

mod legacy {
    use super::*;

    pub(super) fn parse(source: &str) -> Result<ast::Module> {
        let tokens = lexer::lex(source)?;
        parser::parse(&tokens)
    }

    pub(super) fn parse_diagnostics(source: &str) -> std::result::Result<ast::Module, Vec<CompileError>> {
        let tokens = lexer::lex(source).map_err(|error| vec![error])?;
        parser::parse_diagnostics(&tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Item;
    use crate::error::Span;

    #[test]
    fn legacy_and_next_frontends_are_independently_routed() {
        let implicit = "module demo\naction main(value: u64) -> u64 { verification return value }";
        assert!(parse(implicit, CellScriptEdition::Edition2026).is_ok());
        let error = parse(implicit, CellScriptEdition::Edition2027).unwrap_err();
        assert!(error.message.contains("has no explicit source"));

        let explicit = "module demo\naction main(witness value: u64) -> u64 { verification return value }";
        assert!(parse(explicit, CellScriptEdition::Edition2027).is_ok());
    }

    #[test]
    fn next_frontend_fails_closed_on_ambiguous_disposition_and_dispatch() {
        let consume = r#"
module demo
resource Token has consume { amount: u64 }
action main(input token: Token) { verification consume token }
"#;
        assert!(parse(consume, CellScriptEdition::Edition2026).is_ok());
        assert!(parse(consume, CellScriptEdition::Edition2027).unwrap_err().message.contains("ambiguous consume"));

        let multiple = r#"
module demo
action first() { verification return }
action second() { verification return }
"#;
        assert!(parse(multiple, CellScriptEdition::Edition2027).unwrap_err().message.contains("SingleEntry"));

        let capability_only = r#"
module demo
resource Token has consume { amount: u64 }
action main(witness value: u64) -> u64 { verification return value }
"#;
        assert!(parse(capability_only, CellScriptEdition::Edition2027).is_ok());
    }

    #[test]
    fn diagnostic_frontend_keeps_source_spans() {
        let source = "module demo\naction main(value: u64) { verification return }";
        let diagnostics = parse_diagnostics(source, CellScriptEdition::Edition2027).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_ne!(diagnostics[0].span, Span::default());
    }

    #[test]
    fn next_frontend_parses_native_type_script_surface() {
        let source = r#"
module demo

resource Token has store, replace, relock {
    owner: Address,
    amount: u64,
}

type_script TokenTransfer on type_group<Token> {
    entry transfer(
        input token: Token from group_input[0],
        witness recipient: Address from group_witness.input_type,
        output next: Token from group_output[0],
    ) {
        verify {
            enforce token.amount > 0
        }

        effects {
            replace token -> next {
                data {
                    owner = same
                    amount = same
                }
                identity = same
                type_script = same
                lock_script = recipient
                capacity = same
                cardinality = one_to_one
            }
        }
    }
}
"#;
        let module = parse(source, CellScriptEdition::Edition2027).unwrap();
        let Item::Action(action) = module.items.last().unwrap() else {
            panic!("expected native Edition 2027 entry to lower to an action");
        };
        let surface = action.next_surface.as_ref().expect("native surface marker");
        assert_eq!(surface.container_name, "TokenTransfer");
        assert_eq!(surface.trigger_type, "Token");
        assert_eq!(surface.verify.len(), 1);
        assert_eq!(surface.replacements.len(), 1);
        assert_eq!(action.params.len(), 2);
        assert_eq!(action.outputs.len(), 1);

        let formatted = crate::fmt::format_default(&module).unwrap();
        assert!(formatted.contains("type_script TokenTransfer on type_group<Token>"));
        assert!(formatted.contains("replace token -> next"));
        let reparsed = parse(&formatted, CellScriptEdition::Edition2027).unwrap();
        assert_eq!(crate::fmt::format_default(&reparsed).unwrap(), formatted);

        assert!(parse(source, CellScriptEdition::Edition2026).is_err());
        let missing_field = source.replace("                    amount = same\n", "");
        assert!(parse(&missing_field, CellScriptEdition::Edition2027).unwrap_err().message.contains("exhaustively list fields"));
        let wrong_ordinal = source.replace("group_output[0]", "group_output[1]");
        assert!(parse(&wrong_ordinal, CellScriptEdition::Edition2027).unwrap_err().message.contains("non-canonical"));
        let mismatched_output = source
            .replace(
                "resource Token has",
                "resource Other has store, replace, relock { owner: Address, amount: u64 }\nresource Token has",
            )
            .replace("output next: Token", "output next: Other");
        assert!(parse(&mismatched_output, CellScriptEdition::Edition2027)
            .unwrap_err()
            .message
            .contains("input/output ports must all use"));
    }

    #[test]
    fn next_frontend_parses_native_lock_script_surface() {
        let source = r#"
module demo

resource Vault has store {
    owner: Address,
}

lock_script VaultOwner on lock_group {
    entry unlock(
        protected vault: Vault from group_input[0],
        lock_args owner: Address from current_script.args,
        witness claimed_owner: Address from group_witness.input_type,
    ) {
        verify {
            enforce vault.owner == owner
            enforce claimed_owner == owner
        }
    }
}
"#;
        let module = parse(source, CellScriptEdition::Edition2027).unwrap();
        let Item::Lock(lock) = module.items.last().unwrap() else {
            panic!("expected native Edition 2027 entry to lower to a lock");
        };
        let surface = lock.next_surface.as_ref().expect("native lock surface marker");
        assert_eq!(surface.container_name, "VaultOwner");
        assert_eq!(surface.verify.len(), 2);
        assert_eq!(lock.params.len(), 3);

        let formatted = crate::fmt::format_default(&module).unwrap();
        assert!(formatted.contains("lock_script VaultOwner on lock_group"));
        assert!(formatted.contains("protected vault: Vault from group_input[0]"));
        assert!(formatted.contains("lock_args owner: Address from current_script.args"));
        let reparsed = parse(&formatted, CellScriptEdition::Edition2027).unwrap();
        assert_eq!(crate::fmt::format_default(&reparsed).unwrap(), formatted);

        assert!(parse(source, CellScriptEdition::Edition2026).is_err());
        let wrong_ordinal = source.replace("group_input[0]", "group_input[1]");
        assert!(parse(&wrong_ordinal, CellScriptEdition::Edition2027).unwrap_err().message.contains("non-canonical"));
        let no_protected = source.replace("        protected vault: Vault from group_input[0],\n", "");
        assert!(parse(&no_protected, CellScriptEdition::Edition2027).unwrap_err().message.contains("exactly one protected"));
    }
}
