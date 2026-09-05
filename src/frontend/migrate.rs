use super::parse;
use crate::ast::{ActionDef, Expr, Item, LockDef, Module, NextLockSurface, ParamSource, Stmt, Type, Visibility};
use crate::edition::CellScriptEdition;
use crate::error::{CompileError, Result, Span};
use crate::lexer;
use crate::lexer::token::TokenKind;
use serde::Serialize;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MigrationKind {
    TypeScript,
    LockScript,
}

impl MigrationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TypeScript => "type-script",
            Self::LockScript => "lock-script",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MigrationCandidate {
    pub schema: String,
    pub source_edition: String,
    pub target_edition: String,
    pub kind: MigrationKind,
    pub source: String,
}

/// Produce a review-only Edition 2027 candidate for the exact bounded subset
/// whose source/target lowerings are covered by differential evidence. Action
/// candidates retain ordinary transaction-absolute authoring: converting them
/// to native group ports would change their accepted transaction set. The
/// function never edits the input and fails before returning a partial candidate.
pub fn migrate_source_to_2027(source: &str) -> Result<MigrationCandidate> {
    let module = parse(source, CellScriptEdition::Edition2026)?;
    if module.items.iter().any(|item| matches!(item, Item::Use(_))) {
        return Err(CompileError::new(
            "Edition 2027 preview migration currently requires one self-contained source module; imported modules need graph-wide migration",
            module.span,
        ));
    }
    let entries =
        module.items.iter().enumerate().filter(|(_, item)| matches!(item, Item::Action(_) | Item::Lock(_))).collect::<Vec<_>>();
    let [(entry_index, entry)] = entries.as_slice() else {
        return Err(CompileError::new("Edition 2027 preview migration requires exactly one legacy action or lock entry", module.span));
    };
    if *entry_index + 1 != module.items.len() {
        return Err(CompileError::new(
            "the migratable entry must be the final declaration so migration does not reorder unrelated source",
            entry_span(entry),
        ));
    }

    let (replacement_item, kind, entry_span) = match entry {
        Item::Action(action) => (Item::Action(migrate_action(&module, action)?), MigrationKind::TypeScript, action.span),
        Item::Lock(lock) => (Item::Lock(migrate_lock(&module, lock)?), MigrationKind::LockScript, lock.span),
        _ => unreachable!("filtered to executable entries"),
    };
    let replacement = format_single_item(&module, replacement_item)?;
    let source_range = executable_source_range(source, entry_span, kind)?;
    let mut candidate = String::with_capacity(source.len() - source_range.len() + replacement.len());
    candidate.push_str(&source[..source_range.start]);
    candidate.push_str(&replacement);
    candidate.push_str(&source[source_range.end..]);
    parse(&candidate, CellScriptEdition::Edition2027).map_err(|error| {
        CompileError::new(format!("generated Edition 2027 candidate failed its frontend contract: {}", error.message), error.span)
    })?;

    Ok(MigrationCandidate {
        schema: "cellscript-source-migration-preview-v1".to_string(),
        source_edition: "2026".to_string(),
        target_edition: "2027".to_string(),
        kind,
        source: candidate,
    })
}

fn migrate_action(module: &Module, action: &ActionDef) -> Result<ActionDef> {
    if action.next_surface.is_some()
        || action.return_type.is_some()
        || !action.state_edges.is_empty()
        || action.effect_declared
        || action.scheduler_hint.is_some()
        || action.doc_comment.is_some()
    {
        return migration_error(
            action.span,
            "legacy action uses return, transition, effect, scheduler, documentation, or native-container syntax outside the bounded migration subset",
        );
    }
    if module.visibility_of(&action.name) != Visibility::LegacyPublic {
        return migration_error(action.span, "explicit entry visibility has no lossless native-container mapping in this preview");
    }
    if action.params.iter().any(|param| !matches!(param.source, ParamSource::Input | ParamSource::Witness)) {
        return migration_error(
            action.span,
            "type-script migration requires every parameter to be explicitly sourced as input or witness",
        );
    }
    if action.params.iter().any(|param| param.is_mut || param.is_ref || param.is_read_ref) {
        return migration_error(
            action.span,
            "mutable, reference, or read-role parameters have no lossless native-container mapping in this preview",
        );
    }
    let input_types = action
        .params
        .iter()
        .filter(|param| param.source == ParamSource::Input)
        .map(|param| named_type(&param.ty, param.span))
        .collect::<Result<Vec<_>>>()?;
    let output_types = action.outputs.iter().map(|output| named_type(&output.ty, output.span)).collect::<Result<Vec<_>>>()?;
    let Some(trigger_type) = input_types.first().map(|ty| (*ty).to_string()) else {
        return migration_error(action.span, "type-script migration requires at least one explicitly sourced input role");
    };
    if output_types.is_empty() || input_types.iter().chain(&output_types).any(|ty| **ty != trigger_type) {
        return migration_error(
            action.span,
            "type-script migration requires non-empty input/output roles using one identical Cell-backed schema",
        );
    }
    let declared_fields = cell_fields(module, &trigger_type, action.span)?;

    let mut cursor = 0usize;
    while let Some(Stmt::Expr(Expr::Require(require))) = action.body.get(cursor) {
        if require.message.is_some() {
            return migration_error(require.span, "Edition 2027 enforce has no accepted custom-message mapping in this preview");
        }
        cursor += 1;
    }

    let mut replacements = 0usize;
    while cursor < action.body.len() {
        let Some(Stmt::Expr(Expr::StdlibCall(transfer))) = action.body.get(cursor) else {
            return migration_error(action.body[cursor].span(), "type-script migration expected an exact lifecycle transfer");
        };
        let Some(Stmt::Expr(Expr::StdlibCall(capacity))) = action.body.get(cursor + 1) else {
            return migration_error(transfer.span, "each migrated transfer must be followed by preserve_capacity");
        };
        if transfer.namespace != "lifecycle" || transfer.name != "transfer" || transfer.args.len() != 3 {
            return migration_error(transfer.span, "only std::lifecycle::transfer(input, output, lock) is migratable");
        }
        let input = identifier(&transfer.args[0], transfer.span, "transfer input")?;
        let output = identifier(&transfer.args[1], transfer.span, "transfer output")?;
        if transfer.preserve_fields != declared_fields {
            return migration_error(transfer.span, "transfer field list must exhaustively match the Cell schema in declaration order");
        }
        if capacity.namespace != "cell"
            || capacity.name != "preserve_capacity"
            || capacity.args.len() != 2
            || !capacity.preserve_fields.is_empty()
            || identifier(&capacity.args[0], capacity.span, "capacity output")? != output
            || identifier(&capacity.args[1], capacity.span, "capacity input")? != input
        {
            return migration_error(capacity.span, "each migrated transfer requires std::cell::preserve_capacity(output, input)");
        }
        replacements += 1;
        cursor += 2;
    }
    if replacements == 0 {
        return migration_error(action.span, "type-script migration requires at least one exhaustive one-to-one transfer");
    }

    // The authoring frontend accepts this structured action directly. Retain
    // its absolute binding contract instead of silently selecting GroupInput
    // and GroupOutput. Native migration requires a separate reviewed change.
    Ok(action.clone())
}

fn migrate_lock(module: &Module, lock: &LockDef) -> Result<LockDef> {
    if lock.next_surface.is_some() || lock.return_type != Type::Bool {
        return migration_error(lock.span, "lock migration requires the legacy bool-returning lock contract");
    }
    if module.visibility_of(&lock.name) != Visibility::LegacyPublic {
        return migration_error(lock.span, "explicit entry visibility has no lossless native-container mapping in this preview");
    }
    if lock.params.iter().any(|param| !matches!(param.source, ParamSource::Protected | ParamSource::Witness | ParamSource::LockArgs)) {
        return migration_error(
            lock.span,
            "lock-script migration requires every parameter to be explicitly sourced as protected, witness, or lock_args",
        );
    }
    if lock.params.iter().any(|param| param.is_mut || param.is_ref || param.is_read_ref) {
        return migration_error(
            lock.span,
            "mutable, reference, or read-role parameters have no lossless native-container mapping in this preview",
        );
    }
    let protected = lock.params.iter().filter(|param| param.source == ParamSource::Protected).collect::<Vec<_>>();
    let [protected] = protected.as_slice() else {
        return migration_error(lock.span, "lock-script migration requires exactly one protected Cell role");
    };
    let protected_type = match &protected.ty {
        Type::Named(name) => name.as_str(),
        Type::Ref(inner) => named_type(inner, protected.span)?,
        _ => return migration_error(protected.span, "protected role must name a Cell-backed schema"),
    };
    cell_fields(module, protected_type, protected.span)?;

    let mut verify = Vec::new();
    for statement in &lock.body {
        let Stmt::Expr(Expr::Require(require)) = statement else {
            return migration_error(statement.span(), "lock-script migration currently accepts only require conditions");
        };
        if require.message.is_some() {
            return migration_error(require.span, "Edition 2027 enforce has no accepted custom-message mapping in this preview");
        }
        verify.push(require.condition.as_ref().clone());
    }
    let mut migrated = lock.clone();
    migrated.next_surface =
        Some(NextLockSurface { container_name: format!("{}Lock", pascal_case(&lock.name)), verify, audits: Vec::new() });
    Ok(migrated)
}

fn named_type(ty: &Type, span: Span) -> Result<&str> {
    let Type::Named(name) = ty else {
        return migration_error(span, "migrated Cell roles must use a named Cell-backed schema");
    };
    Ok(name)
}

fn cell_fields(module: &Module, name: &str, span: Span) -> Result<Vec<String>> {
    module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Resource(definition) if definition.name == name => Some(&definition.fields),
            Item::Shared(definition) if definition.name == name => Some(&definition.fields),
            Item::Receipt(definition) if definition.name == name => Some(&definition.fields),
            _ => None,
        })
        .map(|fields| fields.iter().map(|field| field.name.clone()).collect())
        .ok_or_else(|| {
            CompileError::new(format!("migrated type_group<{name}> is not declared as a Cell-backed type in this module"), span)
        })
}

fn identifier(expr: &Expr, span: Span, role: &str) -> Result<String> {
    let Expr::Identifier(name) = expr else {
        return migration_error(span, &format!("{role} must be a direct role binding"));
    };
    Ok(name.clone())
}

fn format_single_item(module: &Module, item: Item) -> Result<String> {
    let one = Module {
        name: module.name.clone(),
        items: vec![item],
        interface_templates: Vec::new(),
        visibilities: Default::default(),
        span: module.span,
    };
    let formatted = crate::fmt::format_default(&one)?;
    let header = format!("module {}\n\n", module.name);
    formatted
        .strip_prefix(&header)
        .map(|body| body.trim_end().to_string())
        .ok_or_else(|| CompileError::new("failed to isolate the generated native Script container", module.span))
}

fn executable_source_range(source: &str, entry_span: Span, kind: MigrationKind) -> Result<Range<usize>> {
    let tokens = lexer::lex(source)?;
    let expected = match kind {
        MigrationKind::TypeScript => TokenKind::Action,
        MigrationKind::LockScript => TokenKind::Lock,
    };
    let start_index = tokens
        .iter()
        .position(|token| token.span.start == entry_span.start && token.kind == expected)
        .ok_or_else(|| CompileError::new("failed to locate the legacy entry token for migration", entry_span))?;
    let mut depth = 0usize;
    let mut opened = false;
    for token in &tokens[start_index..] {
        match token.kind {
            TokenKind::LBrace => {
                depth += 1;
                opened = true;
            }
            TokenKind::RBrace if opened => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(entry_span.start..token.span.end);
                }
            }
            _ => {}
        }
    }
    Err(CompileError::new("failed to find the end of the legacy entry for migration", entry_span))
}

fn entry_span(item: &Item) -> Span {
    match item {
        Item::Action(action) => action.span,
        Item::Lock(lock) => lock.span,
        _ => Span::default(),
    }
}

fn pascal_case(name: &str) -> String {
    let mut output = String::new();
    let mut uppercase = true;
    for ch in name.chars() {
        if ch == '_' || ch == '-' {
            uppercase = true;
        } else if uppercase {
            output.extend(ch.to_uppercase());
            uppercase = false;
        } else {
            output.push(ch);
        }
    }
    output
}

fn migration_error<T>(span: Span, message: &str) -> Result<T> {
    Err(CompileError::new(format!("Edition 2027 preview migration stopped: {message}"), span))
}

trait StatementSpan {
    fn span(&self) -> Span;
}

impl StatementSpan for Stmt {
    fn span(&self) -> Span {
        match self {
            Stmt::Let(statement) => statement.span,
            Stmt::Expr(expression) => expression.span(),
            Stmt::Return(statement) => statement.span,
            Stmt::If(statement) => statement.span,
            Stmt::For(statement) => statement.span,
            Stmt::While(statement) => statement.span,
            Stmt::Break(statement) | Stmt::Continue(statement) => statement.span,
            Stmt::Borrow(statement) => statement.span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile, CompileOptions};

    const LEGACY_TYPE: &str = r#"module migrate_type

resource Token has store, replace, relock {
    owner: Address,
    amount: u64,
}

action transfer(input token: Token, witness recipient: Address) -> next: Token {
    verification
        require token.amount > 0
        std::lifecycle::transfer(token, next, recipient) { owner amount }
        std::cell::preserve_capacity(next, token)
}
"#;

    const LEGACY_LOCK: &str = r#"module migrate_lock

resource Vault has store {
    owner: Address,
}

lock unlock(protected vault: Vault, lock_args owner: Address, witness claimed_owner: Address) -> bool {
    verification
        require vault.owner == owner
        require claimed_owner == owner
}
"#;

    #[test]
    fn migrates_only_the_legacy_type_entry() {
        let candidate = migrate_source_to_2027(LEGACY_TYPE).unwrap();
        assert_eq!(candidate.kind, MigrationKind::TypeScript);
        assert!(candidate.source.starts_with(&LEGACY_TYPE[..LEGACY_TYPE.find("action transfer").unwrap()]));
        assert!(candidate.source.contains("action transfer("));
        assert!(candidate.source.contains("require token.amount > 0"));
        assert!(candidate.source.contains("std::lifecycle::transfer(token, next, recipient)"));
        assert!(!candidate.source.contains("type_script"));
        let legacy = compile(
            LEGACY_TYPE,
            CompileOptions {
                edition: CellScriptEdition::Edition2026,
                target: Some("riscv64-elf".to_string()),
                ..CompileOptions::default()
            },
        )
        .unwrap();
        let migrated = compile(
            &candidate.source,
            CompileOptions {
                edition: CellScriptEdition::Edition2027,
                target: Some("riscv64-elf".to_string()),
                ..CompileOptions::default()
            },
        )
        .unwrap();
        assert_eq!(legacy.artifact_bytes, migrated.artifact_bytes);
        assert_eq!(
            legacy.metadata.typed_semantics.foundation.identities.core_semantic_id,
            migrated.metadata.typed_semantics.foundation.identities.core_semantic_id
        );
    }

    #[test]
    fn migrates_only_the_legacy_lock_entry() {
        let candidate = migrate_source_to_2027(LEGACY_LOCK).unwrap();
        assert_eq!(candidate.kind, MigrationKind::LockScript);
        assert!(candidate.source.starts_with(&LEGACY_LOCK[..LEGACY_LOCK.find("lock unlock").unwrap()]));
        assert!(candidate.source.contains("lock_script UnlockLock on lock_group"));
        assert!(candidate.source.contains("protected vault: Vault from group_input[0]"));
        let legacy = compile(
            LEGACY_LOCK,
            CompileOptions {
                edition: CellScriptEdition::Edition2026,
                target: Some("riscv64-elf".to_string()),
                ..CompileOptions::default()
            },
        )
        .unwrap();
        let migrated = compile(
            &candidate.source,
            CompileOptions {
                edition: CellScriptEdition::Edition2027,
                target: Some("riscv64-elf".to_string()),
                ..CompileOptions::default()
            },
        )
        .unwrap();
        assert_eq!(legacy.artifact_bytes, migrated.artifact_bytes);
        assert_eq!(legacy.metadata.typed_semantics.foundation.identities, migrated.metadata.typed_semantics.foundation.identities);
    }

    #[test]
    fn rejects_ambiguous_or_lossy_legacy_source() {
        let ambiguous = "module demo\nresource Token has consume { amount: u64 }\naction main(input token: Token) -> next: Token { verification consume token }";
        assert!(migrate_source_to_2027(ambiguous).unwrap_err().message.contains("expected an exact lifecycle transfer"));
        let message = LEGACY_LOCK.replace("require vault.owner == owner", "require vault.owner == owner, \"owner mismatch\"");
        assert!(migrate_source_to_2027(&message).unwrap_err().message.contains("no accepted custom-message mapping"));
        let visibility = LEGACY_TYPE.replace("action transfer", "private action transfer");
        assert!(migrate_source_to_2027(&visibility).unwrap_err().message.contains("explicit entry visibility"));
    }
}
