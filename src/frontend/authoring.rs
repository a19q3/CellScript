//! Familiar authoring grammar over the shared value/statement kernel.
//!
//! This is an independent frontend policy, not a translation into preview4
//! text. Existing 2026 declarations and expressions keep their checked meaning.
//! In particular, a legacy terminal consume is not reclassified as retirement,
//! and a parameter's source is resolved by the existing typed entry contract.
//! Multiple source entries do not imply runtime dispatch: artifact selection
//! remains a separate, explicitly verified boundary.

use crate::ast::Module;
use crate::error::{CompileError, Result};
use crate::lexer::token::Token;
use crate::parser::{self, EntryBodyGrammar};

pub(super) fn parse(tokens: &[Token]) -> Result<Module> {
    parser::parse_with_entry_grammar(tokens, EntryBodyGrammar::ConstraintBlock)
}

pub(super) fn parse_diagnostics(tokens: &[Token]) -> std::result::Result<Module, Vec<CompileError>> {
    parser::parse_diagnostics_with_entry_grammar(tokens, EntryBodyGrammar::ConstraintBlock)
}
