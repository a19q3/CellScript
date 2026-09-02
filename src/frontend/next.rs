use super::*;
use crate::ast::{
    ActionDef, ActionOutput, EffectClass, Expr, Item, LockDef, NextEntrySurface, NextLockSurface, NextReplacement, Param, ParamSource,
    RequireExpr, Stmt, Type, Visibility,
};
use crate::error::Span;
use crate::lexer::token::{Token, TokenKind};
use std::collections::BTreeSet;

pub(super) fn parse(source: &str) -> Result<ast::Module> {
    let tokens = lexer::lex(source)?;
    let module = if has_native_surface(&tokens) { parse_native_surface(&tokens)? } else { parser::parse(&tokens)? };
    let diagnostics = constitution_diagnostics(&module, &tokens);
    diagnostics.into_iter().next().map_or(Ok(module), Err)
}

pub(super) fn parse_diagnostics(source: &str) -> std::result::Result<ast::Module, Vec<CompileError>> {
    let tokens = lexer::lex(source).map_err(|error| vec![error])?;
    let module = if has_native_surface(&tokens) {
        parse_native_surface(&tokens).map_err(|error| vec![error])?
    } else {
        parser::parse_diagnostics(&tokens)?
    };
    let diagnostics = constitution_diagnostics(&module, &tokens);
    if diagnostics.is_empty() {
        Ok(module)
    } else {
        Err(diagnostics)
    }
}

fn has_native_surface(tokens: &[Token]) -> bool {
    let mut depth = 0usize;
    for token in tokens {
        match token.kind {
            TokenKind::LBrace => depth = depth.saturating_add(1),
            TokenKind::RBrace => depth = depth.saturating_sub(1),
            _ if depth == 0 && (token_is_word(token, "type_script") || token_is_word(token, "lock_script")) => return true,
            _ => {}
        }
    }
    false
}

fn parse_native_surface(tokens: &[Token]) -> Result<ast::Module> {
    let type_positions = top_level_word_positions(tokens, "type_script");
    let lock_positions = top_level_word_positions(tokens, "lock_script");
    if type_positions.len() + lock_positions.len() != 1 {
        let span =
            type_positions.first().or_else(|| lock_positions.first()).map_or_else(Span::default, |position| tokens[*position].span);
        return Err(CompileError::new(
            "Edition 2027 currently requires exactly one native type_script or lock_script container per source module",
            span,
        ));
    }
    if type_positions.len() == 1 {
        parse_type_script_surface(tokens)
    } else {
        parse_lock_script_surface(tokens)
    }
}

fn parse_type_script_surface(tokens: &[Token]) -> Result<ast::Module> {
    let positions = top_level_word_positions(tokens, "type_script");
    if positions.len() != 1 {
        return Err(CompileError::new(
            "Edition 2027 currently requires exactly one type_script container per source module",
            positions.first().map_or_else(Span::default, |position| tokens[*position].span),
        ));
    }
    let start = positions[0];
    let end = matching_container_end(tokens, start)?;
    if tokens[end + 1..].iter().any(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof)) {
        return Err(CompileError::new(
            "Edition 2027 type_script must be the final top-level declaration in this preview",
            tokens[end + 1].span,
        ));
    }

    let mut base_tokens = tokens[..start].to_vec();
    let eof = tokens.last().cloned().unwrap_or_else(|| Token::new(TokenKind::Eof, Span::default(), ""));
    base_tokens.push(eof);
    let mut module = parser::parse(&base_tokens)?;
    if module.items.iter().any(|item| matches!(item, Item::Action(_) | Item::Lock(_))) {
        return Err(CompileError::new(
            "Edition 2027 type_script cannot be combined with legacy action or lock declarations",
            tokens[start].span,
        ));
    }

    let mut cursor = Cursor::new(&tokens[start..=end]);
    let action = cursor.parse_type_script(&module)?;
    cursor.skip_newlines();
    if !cursor.is_done() {
        return Err(CompileError::new("unexpected tokens after Edition 2027 type_script", cursor.span()));
    }
    module.visibilities.insert(action.name.clone(), Visibility::LegacyPublic);
    module.items.push(Item::Action(action));
    module.span = module.span.combine(&tokens[end].span);
    Ok(module)
}

fn parse_lock_script_surface(tokens: &[Token]) -> Result<ast::Module> {
    let positions = top_level_word_positions(tokens, "lock_script");
    debug_assert_eq!(positions.len(), 1);
    let start = positions[0];
    let end = matching_container_end(tokens, start)?;
    if tokens[end + 1..].iter().any(|token| !matches!(token.kind, TokenKind::Newline | TokenKind::Eof)) {
        return Err(CompileError::new(
            "Edition 2027 lock_script must be the final top-level declaration in this preview",
            tokens[end + 1].span,
        ));
    }

    let mut base_tokens = tokens[..start].to_vec();
    let eof = tokens.last().cloned().unwrap_or_else(|| Token::new(TokenKind::Eof, Span::default(), ""));
    base_tokens.push(eof);
    let mut module = parser::parse(&base_tokens)?;
    if module.items.iter().any(|item| matches!(item, Item::Action(_) | Item::Lock(_))) {
        return Err(CompileError::new(
            "Edition 2027 lock_script cannot be combined with legacy action or lock declarations",
            tokens[start].span,
        ));
    }

    let mut cursor = Cursor::new(&tokens[start..=end]);
    let lock = cursor.parse_lock_script(&module)?;
    cursor.skip_newlines();
    if !cursor.is_done() {
        return Err(CompileError::new("unexpected tokens after Edition 2027 lock_script", cursor.span()));
    }
    module.visibilities.insert(lock.name.clone(), Visibility::LegacyPublic);
    module.items.push(Item::Lock(lock));
    module.span = module.span.combine(&tokens[end].span);
    Ok(module)
}

fn top_level_word_positions(tokens: &[Token], word: &str) -> Vec<usize> {
    let mut depth = 0usize;
    let mut positions = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::LBrace => depth = depth.saturating_add(1),
            TokenKind::RBrace => depth = depth.saturating_sub(1),
            _ if depth == 0 && token_is_word(token, word) => positions.push(index),
            _ => {}
        }
    }
    positions
}

fn matching_container_end(tokens: &[Token], start: usize) -> Result<usize> {
    let open = tokens[start..]
        .iter()
        .position(|token| matches!(token.kind, TokenKind::LBrace))
        .map(|offset| start + offset)
        .ok_or_else(|| CompileError::new("native Script container requires a body", tokens[start].span))?;
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token.kind {
            TokenKind::LBrace => depth = depth.saturating_add(1),
            TokenKind::RBrace => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
    }
    Err(CompileError::new("unterminated native Script container body", tokens[open].span))
}

struct Cursor<'a> {
    tokens: &'a [Token],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, position: 0 }
    }

    fn current(&self) -> Option<&'a Token> {
        self.tokens.get(self.position)
    }

    fn span(&self) -> Span {
        self.current().map_or_else(|| self.tokens.last().map_or_else(Span::default, |token| token.span), |token| token.span)
    }

    fn is_done(&self) -> bool {
        self.position >= self.tokens.len()
    }

    fn advance(&mut self) -> Option<&'a Token> {
        let token = self.current();
        self.position = self.position.saturating_add(1);
        token
    }

    fn skip_newlines(&mut self) {
        while self.current().is_some_and(|token| matches!(token.kind, TokenKind::Newline)) {
            self.advance();
        }
    }

    fn consume_statement_separator(&mut self) {
        if self.current().is_some_and(|token| matches!(token.kind, TokenKind::Semi)) {
            self.advance();
        }
        self.skip_newlines();
    }

    fn at_kind(&self, expected: &TokenKind) -> bool {
        self.current().is_some_and(|token| std::mem::discriminant(&token.kind) == std::mem::discriminant(expected))
    }

    fn expect_kind(&mut self, expected: TokenKind) -> Result<Token> {
        let Some(token) = self.current() else {
            return Err(CompileError::new(format!("expected {expected}, found end of native Script container"), self.span()));
        };
        if std::mem::discriminant(&token.kind) != std::mem::discriminant(&expected) {
            return Err(CompileError::new(format!("expected {expected}, found {}", token.kind), token.span));
        }
        let token = token.clone();
        self.advance();
        Ok(token)
    }

    fn expect_word(&mut self, word: &str) -> Result<Token> {
        let Some(token) = self.current() else {
            return Err(CompileError::new(format!("expected '{word}', found end of native Script container"), self.span()));
        };
        if !token_is_word(token, word) {
            return Err(CompileError::new(format!("expected '{word}', found {}", token.kind), token.span));
        }
        let token = token.clone();
        self.advance();
        Ok(token)
    }

    fn parse_name(&mut self, context: &str) -> Result<String> {
        let Some(token) = self.current() else {
            return Err(CompileError::new(format!("expected {context}, found end of native Script container"), self.span()));
        };
        let TokenKind::Identifier(name) = &token.kind else {
            return Err(CompileError::new(format!("expected {context}, found {}", token.kind), token.span));
        };
        let name = name.clone();
        self.advance();
        Ok(name)
    }

    fn parse_type_script(&mut self, module: &ast::Module) -> Result<ActionDef> {
        let start = self.expect_word("type_script")?.span;
        let container_name = self.parse_name("type_script name")?;
        self.expect_word("on")?;
        self.expect_word("type_group")?;
        self.expect_kind(TokenKind::Lt)?;
        let trigger_type = self.parse_name("type_group schema")?;
        self.expect_kind(TokenKind::Gt)?;
        self.skip_newlines();
        self.expect_kind(TokenKind::LBrace)?;
        self.skip_newlines();
        self.expect_word("entry")?;
        let entry_name = self.parse_name("entry name")?;
        let (params, outputs) = self.parse_ports()?;
        self.skip_newlines();
        self.expect_kind(TokenKind::LBrace)?;
        self.skip_newlines();
        let (verify, mut body) = self.parse_verify()?;
        self.skip_newlines();
        let (replacements, effects) = self.parse_effects()?;
        body.extend(effects);
        self.skip_newlines();
        let entry_end = self.expect_kind(TokenKind::RBrace)?.span;
        self.skip_newlines();
        let container_end = self.expect_kind(TokenKind::RBrace)?.span;

        validate_surface(module, &trigger_type, &params, &outputs, &replacements, start)?;
        Ok(ActionDef {
            name: entry_name,
            params,
            return_type: None,
            outputs,
            state_edges: Vec::new(),
            body,
            effect: EffectClass::Pure,
            effect_declared: false,
            scheduler_hint: None,
            next_surface: Some(NextEntrySurface { container_name, trigger_type, verify, replacements }),
            doc_comment: None,
            span: Span::new(start.start, container_end.end, start.line, start.column).combine(&entry_end),
        })
    }

    fn parse_lock_script(&mut self, module: &ast::Module) -> Result<LockDef> {
        let start = self.expect_word("lock_script")?.span;
        let container_name = self.parse_name("lock_script name")?;
        self.expect_word("on")?;
        self.expect_word("lock_group")?;
        self.skip_newlines();
        self.expect_kind(TokenKind::LBrace)?;
        self.skip_newlines();
        self.expect_word("entry")?;
        let entry_name = self.parse_name("entry name")?;
        let params = self.parse_lock_ports(module)?;
        self.skip_newlines();
        self.expect_kind(TokenKind::LBrace)?;
        self.skip_newlines();
        let (verify, body) = self.parse_verify()?;
        self.skip_newlines();
        let entry_end = self.expect_kind(TokenKind::RBrace)?.span;
        self.skip_newlines();
        let container_end = self.expect_kind(TokenKind::RBrace)?.span;
        Ok(LockDef {
            name: entry_name,
            params,
            return_type: Type::Bool,
            body,
            next_surface: Some(NextLockSurface { container_name, verify }),
            span: Span::new(start.start, container_end.end, start.line, start.column).combine(&entry_end),
        })
    }

    fn parse_lock_ports(&mut self, module: &ast::Module) -> Result<Vec<Param>> {
        self.expect_kind(TokenKind::LParen)?;
        self.skip_newlines();
        let mut params = Vec::new();
        let mut names = BTreeSet::new();
        let mut protected_ordinal = 0usize;
        while !self.at_kind(&TokenKind::RParen) {
            let start = self.span();
            let source_word = self.parse_name("lock port source (protected, witness, or lock_args)")?;
            let source = match source_word.as_str() {
                "protected" => ParamSource::Protected,
                "witness" => ParamSource::Witness,
                "lock_args" => ParamSource::LockArgs,
                _ => {
                    return Err(CompileError::new(
                        "Edition 2027 lock_script ports currently support protected, witness, and lock_args",
                        start,
                    ));
                }
            };
            let name = self.parse_name("lock port binding")?;
            if !names.insert(name.clone()) {
                return Err(CompileError::new(format!("duplicate entry port '{name}'"), start));
            }
            self.expect_kind(TokenKind::Colon)?;
            let type_start = self.position;
            let mut depth = 0usize;
            while let Some(token) = self.current() {
                match token.kind {
                    TokenKind::Lt | TokenKind::LBracket | TokenKind::LParen => {
                        depth = depth.saturating_add(1);
                        self.advance();
                    }
                    TokenKind::Gt | TokenKind::RBracket | TokenKind::RParen if depth > 0 => {
                        depth = depth.saturating_sub(1);
                        self.advance();
                    }
                    _ if depth == 0 && token_is_word(token, "from") => break,
                    _ => {
                        self.advance();
                    }
                }
            }
            if type_start == self.position {
                return Err(CompileError::new("lock entry port requires a type", self.span()));
            }
            let mut ty = parser::parse_type_fragment(&self.tokens[type_start..self.position])?;
            self.expect_word("from")?;
            match source {
                ParamSource::Protected => {
                    self.parse_indexed_source("group_input", protected_ordinal)?;
                    protected_ordinal += 1;
                    validate_protected_type(module, &ty, start)?;
                    ty = Type::Ref(Box::new(ty));
                }
                ParamSource::Witness => {
                    self.expect_word("group_witness")?;
                    self.expect_kind(TokenKind::Dot)?;
                    self.expect_word("input_type")?;
                }
                ParamSource::LockArgs => {
                    self.expect_word("current_script")?;
                    self.expect_kind(TokenKind::Dot)?;
                    self.expect_word("args")?;
                }
                ParamSource::Default | ParamSource::Input | ParamSource::Output => unreachable!(),
            }
            params.push(Param {
                name,
                ty,
                is_mut: false,
                is_ref: false,
                is_read_ref: false,
                source,
                span: start.combine(&self.span()),
            });
            self.skip_newlines();
            if self.at_kind(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            } else if !self.at_kind(&TokenKind::RParen) {
                return Err(CompileError::new("expected ',' or ')' after lock entry port", self.span()));
            }
        }
        self.expect_kind(TokenKind::RParen)?;
        if protected_ordinal != 1 {
            return Err(CompileError::new(
                "Edition 2027 lock_script preview requires exactly one protected group_input[0] role",
                self.span(),
            ));
        }
        Ok(params)
    }

    fn parse_ports(&mut self) -> Result<(Vec<Param>, Vec<ActionOutput>)> {
        self.expect_kind(TokenKind::LParen)?;
        self.skip_newlines();
        let mut params = Vec::new();
        let mut outputs = Vec::new();
        let mut names = BTreeSet::new();
        let mut input_ordinal = 0usize;
        let mut output_ordinal = 0usize;
        while !self.at_kind(&TokenKind::RParen) {
            let start = self.span();
            let source_word = self.parse_name("port source (input, output, or witness)")?;
            let source = match source_word.as_str() {
                "input" => ParamSource::Input,
                "output" => ParamSource::Output,
                "witness" => ParamSource::Witness,
                _ => {
                    return Err(CompileError::new(
                        "Edition 2027 type_script ports currently support input, output, and witness",
                        start,
                    ));
                }
            };
            let name = self.parse_name("port binding")?;
            if !names.insert(name.clone()) {
                return Err(CompileError::new(format!("duplicate entry port '{name}'"), start));
            }
            self.expect_kind(TokenKind::Colon)?;
            let type_start = self.position;
            let mut depth = 0usize;
            while let Some(token) = self.current() {
                match token.kind {
                    TokenKind::Lt | TokenKind::LBracket | TokenKind::LParen => {
                        depth = depth.saturating_add(1);
                        self.advance();
                    }
                    TokenKind::Gt | TokenKind::RBracket | TokenKind::RParen if depth > 0 => {
                        depth = depth.saturating_sub(1);
                        self.advance();
                    }
                    _ if depth == 0 && token_is_word(token, "from") => break,
                    _ => {
                        self.advance();
                    }
                }
            }
            if type_start == self.position {
                return Err(CompileError::new("entry port requires a type", self.span()));
            }
            let ty = parser::parse_type_fragment(&self.tokens[type_start..self.position])?;
            self.expect_word("from")?;
            match source {
                ParamSource::Input => {
                    self.parse_indexed_source("group_input", input_ordinal)?;
                    input_ordinal += 1;
                    params.push(Param {
                        name,
                        ty,
                        is_mut: false,
                        is_ref: false,
                        is_read_ref: false,
                        source,
                        span: start.combine(&self.span()),
                    });
                }
                ParamSource::Output => {
                    self.parse_indexed_source("group_output", output_ordinal)?;
                    output_ordinal += 1;
                    outputs.push(ActionOutput { name, ty, span: start.combine(&self.span()) });
                }
                ParamSource::Witness => {
                    self.expect_word("group_witness")?;
                    self.expect_kind(TokenKind::Dot)?;
                    self.expect_word("input_type")?;
                    params.push(Param {
                        name,
                        ty,
                        is_mut: false,
                        is_ref: false,
                        is_read_ref: false,
                        source,
                        span: start.combine(&self.span()),
                    });
                }
                ParamSource::Default | ParamSource::Protected | ParamSource::LockArgs => unreachable!(),
            }
            self.skip_newlines();
            if self.at_kind(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            } else if !self.at_kind(&TokenKind::RParen) {
                return Err(CompileError::new("expected ',' or ')' after entry port", self.span()));
            }
        }
        self.expect_kind(TokenKind::RParen)?;
        Ok((params, outputs))
    }

    fn parse_indexed_source(&mut self, source: &str, expected: usize) -> Result<()> {
        self.expect_word(source)?;
        self.expect_kind(TokenKind::LBracket)?;
        let ordinal = match self.current().map(|token| &token.kind) {
            Some(TokenKind::Integer(value)) => usize::try_from(*value)
                .map_err(|_| CompileError::new(format!("{source} ordinal exceeds the supported index range"), self.span()))?,
            _ => return Err(CompileError::new(format!("{source} requires a numeric ordinal"), self.span())),
        };
        if ordinal != expected {
            return Err(CompileError::new(
                format!("{source}[{ordinal}] is non-canonical here; expected {source}[{expected}]"),
                self.span(),
            ));
        }
        self.advance();
        self.expect_kind(TokenKind::RBracket)?;
        Ok(())
    }

    fn parse_verify(&mut self) -> Result<(Vec<Expr>, Vec<Stmt>)> {
        self.expect_word("verify")?;
        self.skip_newlines();
        self.expect_kind(TokenKind::LBrace)?;
        self.skip_newlines();
        let mut verify = Vec::new();
        let mut body = Vec::new();
        while !self.at_kind(&TokenKind::RBrace) {
            let enforce = self.expect_word("enforce")?.span;
            let expression_tokens = self.take_statement_tokens()?;
            if expression_tokens.is_empty() {
                return Err(CompileError::new("enforce requires a boolean expression", enforce));
            }
            let expression = parser::parse_expression_fragment(expression_tokens)?;
            let span = enforce.combine(&expression.span());
            verify.push(expression.clone());
            body.push(Stmt::Expr(Expr::Require(RequireExpr { condition: Box::new(expression), message: None, span })));
            self.consume_statement_separator();
        }
        self.expect_kind(TokenKind::RBrace)?;
        Ok((verify, body))
    }

    fn parse_effects(&mut self) -> Result<(Vec<NextReplacement>, Vec<Stmt>)> {
        self.expect_word("effects")?;
        self.skip_newlines();
        self.expect_kind(TokenKind::LBrace)?;
        self.skip_newlines();
        let mut replacements = Vec::new();
        let mut body = Vec::new();
        while !self.at_kind(&TokenKind::RBrace) {
            let replacement = self.parse_replacement()?;
            let span = replacement.span;
            body.push(Stmt::Expr(Expr::StdlibCall(crate::ast::StdlibCallExpr {
                namespace: "lifecycle".to_string(),
                name: "transfer".to_string(),
                args: vec![
                    Expr::Identifier(replacement.input.clone()),
                    Expr::Identifier(replacement.output.clone()),
                    replacement.lock_script.clone(),
                ],
                preserve_fields: replacement.data_fields.clone(),
                span,
            })));
            body.push(Stmt::Expr(Expr::StdlibCall(crate::ast::StdlibCallExpr {
                namespace: "cell".to_string(),
                name: "preserve_capacity".to_string(),
                args: vec![Expr::Identifier(replacement.output.clone()), Expr::Identifier(replacement.input.clone())],
                preserve_fields: Vec::new(),
                span,
            })));
            replacements.push(replacement);
            self.consume_statement_separator();
        }
        if replacements.is_empty() {
            return Err(CompileError::new("effects must contain at least one exhaustive disposition", self.span()));
        }
        self.expect_kind(TokenKind::RBrace)?;
        Ok((replacements, body))
    }

    fn parse_replacement(&mut self) -> Result<NextReplacement> {
        let start = self.expect_word("replace")?.span;
        let input = self.parse_name("replacement input role")?;
        self.expect_kind(TokenKind::Arrow)?;
        let output = self.parse_name("replacement output role")?;
        self.skip_newlines();
        self.expect_kind(TokenKind::LBrace)?;
        self.skip_newlines();
        self.expect_word("data")?;
        self.skip_newlines();
        self.expect_kind(TokenKind::LBrace)?;
        self.skip_newlines();
        let mut data_fields = Vec::new();
        while !self.at_kind(&TokenKind::RBrace) {
            let field = self.parse_name("data field")?;
            self.expect_kind(TokenKind::Eq)?;
            self.expect_word("same")?;
            data_fields.push(field);
            self.consume_statement_separator();
            if self.at_kind(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            }
        }
        self.expect_kind(TokenKind::RBrace)?;
        self.consume_statement_separator();
        self.parse_same_property("identity")?;
        self.parse_same_property("type_script")?;
        self.expect_word("lock_script")?;
        self.expect_kind(TokenKind::Eq)?;
        let lock_tokens = self.take_statement_tokens()?;
        if lock_tokens.is_empty() {
            return Err(CompileError::new("lock_script requires an Address expression", self.span()));
        }
        let lock_script = parser::parse_expression_fragment(lock_tokens)?;
        self.consume_statement_separator();
        self.parse_same_property("capacity")?;
        self.expect_word("cardinality")?;
        self.expect_kind(TokenKind::Eq)?;
        self.expect_word("one_to_one")?;
        self.consume_statement_separator();
        let end = self.expect_kind(TokenKind::RBrace)?.span;
        Ok(NextReplacement {
            input,
            output,
            data_fields,
            lock_script,
            span: Span::new(start.start, end.end, start.line, start.column),
        })
    }

    fn parse_same_property(&mut self, property: &str) -> Result<()> {
        self.expect_word(property)?;
        self.expect_kind(TokenKind::Eq)?;
        self.expect_word("same")?;
        self.consume_statement_separator();
        Ok(())
    }

    fn take_statement_tokens(&mut self) -> Result<&'a [Token]> {
        let start = self.position;
        let mut paren = 0usize;
        let mut bracket = 0usize;
        let mut brace = 0usize;
        while let Some(token) = self.current() {
            match token.kind {
                TokenKind::LParen => paren = paren.saturating_add(1),
                TokenKind::RParen if paren > 0 => paren = paren.saturating_sub(1),
                TokenKind::LBracket => bracket = bracket.saturating_add(1),
                TokenKind::RBracket if bracket > 0 => bracket = bracket.saturating_sub(1),
                TokenKind::LBrace => brace = brace.saturating_add(1),
                TokenKind::RBrace if brace > 0 => brace = brace.saturating_sub(1),
                TokenKind::Newline | TokenKind::Semi if paren == 0 && bracket == 0 && brace == 0 => break,
                TokenKind::RBrace if paren == 0 && bracket == 0 && brace == 0 => break,
                _ => {}
            }
            self.advance();
        }
        if self.position == start {
            return Ok(&self.tokens[start..start]);
        }
        Ok(&self.tokens[start..self.position])
    }
}

fn validate_surface(
    module: &ast::Module,
    trigger_type: &str,
    params: &[Param],
    outputs: &[ActionOutput],
    replacements: &[NextReplacement],
    span: Span,
) -> Result<()> {
    let fields = module.items.iter().find_map(|item| match item {
        Item::Resource(definition) if definition.name == trigger_type => Some(&definition.fields),
        Item::Shared(definition) if definition.name == trigger_type => Some(&definition.fields),
        Item::Receipt(definition) if definition.name == trigger_type => Some(&definition.fields),
        _ => None,
    });
    let Some(fields) = fields else {
        return Err(CompileError::new(
            format!("type_group<{trigger_type}> must name a Cell-backed resource, shared type, or receipt in this module"),
            span,
        ));
    };
    let trigger_ty = Type::Named(trigger_type.to_string());
    if params.iter().any(|param| param.source == ParamSource::Input && param.ty != trigger_ty)
        || outputs.iter().any(|output| output.ty != trigger_ty)
    {
        return Err(CompileError::new(
            format!("Edition 2027 preview input/output ports must all use the declared type_group<{trigger_type}> schema"),
            span,
        ));
    }
    let declared_fields = fields.iter().map(|field| field.name.clone()).collect::<Vec<_>>();
    let inputs = params
        .iter()
        .filter(|param| param.source == ParamSource::Input && param.ty == Type::Named(trigger_type.to_string()))
        .map(|param| param.name.as_str())
        .collect::<BTreeSet<_>>();
    let output_names = outputs
        .iter()
        .filter(|output| output.ty == Type::Named(trigger_type.to_string()))
        .map(|output| output.name.as_str())
        .collect::<BTreeSet<_>>();
    if inputs.is_empty() || output_names.is_empty() {
        return Err(CompileError::new(
            format!("type_group<{trigger_type}> entry requires explicit input and output roles of type {trigger_type}"),
            span,
        ));
    }
    let mut disposed_inputs = BTreeSet::new();
    let mut produced_outputs = BTreeSet::new();
    for replacement in replacements {
        if !inputs.contains(replacement.input.as_str()) || !output_names.contains(replacement.output.as_str()) {
            return Err(CompileError::new(
                format!(
                    "replace {} -> {} must bind declared type_group<{trigger_type}> input/output roles",
                    replacement.input, replacement.output
                ),
                replacement.span,
            ));
        }
        if !disposed_inputs.insert(replacement.input.as_str()) || !produced_outputs.insert(replacement.output.as_str()) {
            return Err(CompileError::new("each input and output role may appear in exactly one disposition", replacement.span));
        }
        if replacement.data_fields != declared_fields {
            return Err(CompileError::new(
                format!("replace data must exhaustively list fields in schema order: {}", declared_fields.join(", ")),
                replacement.span,
            ));
        }
    }
    if disposed_inputs != inputs || produced_outputs != output_names {
        return Err(CompileError::new(
            "effects must exhaustively dispose every type-group input and account for every type-group output",
            span,
        ));
    }
    Ok(())
}

fn validate_protected_type(module: &ast::Module, ty: &Type, span: Span) -> Result<()> {
    let Type::Named(name) = ty else {
        return Err(CompileError::new("Edition 2027 protected ports must name a Cell-backed resource, shared type, or receipt", span));
    };
    let cell_backed = module.items.iter().any(|item| {
        matches!(item, Item::Resource(definition) if definition.name == *name)
            || matches!(item, Item::Shared(definition) if definition.name == *name)
            || matches!(item, Item::Receipt(definition) if definition.name == *name)
    });
    if !cell_backed {
        return Err(CompileError::new(format!("protected port type {name} is not a Cell-backed declaration in this module"), span));
    }
    Ok(())
}

fn constitution_diagnostics(module: &ast::Module, tokens: &[Token]) -> Vec<CompileError> {
    let mut diagnostics = Vec::new();
    let entries = module.items.iter().filter(|item| matches!(item, Item::Action(_) | Item::Lock(_))).collect::<Vec<_>>();
    if entries.len() > 1 {
        diagnostics.push(CompileError::new(
            "Edition 2027 preview emits only SingleEntry artifacts; select one source entry or split the artifact until explicit versioned dispatch is accepted",
            module.span,
        ));
    }
    for item in &module.items {
        let (entry_kind, entry_name, params) = match item {
            Item::Action(entry) => ("action", entry.name.as_str(), entry.params.as_slice()),
            Item::Lock(entry) => ("lock", entry.name.as_str(), entry.params.as_slice()),
            _ => continue,
        };
        for param in params {
            if param.source == ParamSource::Default {
                diagnostics.push(CompileError::new(
                    format!(
                        "Edition 2027 parameter '{}::{}' has no explicit source; use input, output, protected, witness, or lock_args provenance",
                        entry_kind, entry_name
                    ),
                    param.span,
                ));
            }
        }
    }
    for (index, token) in tokens.iter().enumerate() {
        let next_significant = tokens[index + 1..]
            .iter()
            .find(|candidate| !matches!(candidate.kind, TokenKind::Whitespace | TokenKind::Comment(_) | TokenKind::Newline));
        let consume_statement = matches!(token.kind, TokenKind::Consume)
            && next_significant.is_some_and(|candidate| {
                !matches!(candidate.kind, TokenKind::Comma | TokenKind::LBrace | TokenKind::RBrace | TokenKind::Semi | TokenKind::Eof)
            });
        let ambiguous_consume = consume_statement || matches!(&token.kind, TokenKind::Identifier(name) if name == "consume_each");
        if ambiguous_consume {
            diagnostics.push(CompileError::new(
                "Edition 2027 rejects ambiguous consume/consume_each disposition; choose an explicit successor, pooled accounting, or retirement policy",
                token.span,
            ));
        }
    }
    diagnostics
}

fn token_is_word(token: &Token, word: &str) -> bool {
    match &token.kind {
        TokenKind::Identifier(name) => name == word,
        _ => token.text == word,
    }
}
