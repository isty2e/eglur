use crate::event::CompletedMarker;
use crate::{SyntaxDiagnosticKind, SyntaxKind, TokenKind};

use super::Parser;

impl Parser<'_> {
    pub(super) fn parse_expression(
        &mut self,
        grouped: bool,
        allow_outer_brace_suffix: bool,
    ) -> Option<CompletedMarker> {
        self.parse_expression_bp(grouped, allow_outer_brace_suffix, 1)
    }

    fn parse_expression_bp(
        &mut self,
        grouped: bool,
        allow_outer_brace_suffix: bool,
        minimum_precedence: u8,
    ) -> Option<CompletedMarker> {
        if !self.enter_recursive() {
            return None;
        }
        let mut left = self.parse_unary(allow_outer_brace_suffix);
        let mut saw_range = false;

        while let Some(completed) = left {
            let (leading_newlines, precedence, is_range) = match self.next_binary_operator(grouped)
            {
                Some(operator) if operator.1 >= minimum_precedence => operator,
                _ => break,
            };
            if is_range && saw_range {
                self.error_current(SyntaxDiagnosticKind::ParseInvalidRangeSyntax);
                break;
            }
            saw_range |= is_range;

            for _ in 0..leading_newlines {
                self.bump();
            }
            let binary = self.precede(completed);
            let operator_span = self.current_span();
            self.bump();
            self.skip_newlines();
            if !self.expression_can_start()
                || (!allow_outer_brace_suffix && self.at(TokenKind::LeftBrace))
            {
                self.error_at(
                    if is_range {
                        SyntaxDiagnosticKind::ParseInvalidRangeSyntax
                    } else {
                        SyntaxDiagnosticKind::ParseExpectedExpression
                    },
                    operator_span,
                );
                left = Some(self.complete(binary, SyntaxKind::BinaryExpr));
                break;
            }
            self.parse_expression_bp(grouped, allow_outer_brace_suffix, precedence + 1);
            left = Some(self.complete(binary, SyntaxKind::BinaryExpr));
            if self.halted() {
                break;
            }
        }

        self.leave_recursive();
        left
    }

    fn parse_unary(&mut self, allow_outer_brace_suffix: bool) -> Option<CompletedMarker> {
        let (kind, requires_identifier) = match self.current_kind() {
            Some(TokenKind::Ampersand) => (SyntaxKind::RefExpr, false),
            Some(TokenKind::OwnKw) => (SyntaxKind::OwnExpr, true),
            Some(TokenKind::MoveKw) => (SyntaxKind::MoveExpr, true),
            Some(TokenKind::DropOwnKw) => (SyntaxKind::DropOwnExpr, true),
            Some(TokenKind::Bang | TokenKind::Minus | TokenKind::Star) => {
                (SyntaxKind::UnaryExpr, false)
            }
            _ => return self.parse_postfix(allow_outer_brace_suffix),
        };

        let unary = self.start();
        let operator = self.current_kind();
        self.bump();
        if operator == Some(TokenKind::Ampersand) {
            self.eat(TokenKind::MutKw);
        }
        if operator == Some(TokenKind::DropOwnKw) {
            self.eat(TokenKind::MoveKw);
        }
        if requires_identifier {
            self.expect_identifier();
        } else if self.enter_recursive() {
            self.parse_unary(allow_outer_brace_suffix);
            self.leave_recursive();
        }
        Some(self.complete(unary, kind))
    }

    fn parse_postfix(&mut self, allow_outer_brace_suffix: bool) -> Option<CompletedMarker> {
        let mut expression = self.parse_primary(allow_outer_brace_suffix);
        while let Some(completed) = expression {
            let leading_newlines = self.newlines_before_dot();
            if leading_newlines > 0 {
                for _ in 0..leading_newlines {
                    self.bump();
                }
            }

            if self.at(TokenKind::Dot) {
                if self.nth_contextual(1, "reduce_by") && self.nth_at(2, TokenKind::LeftParen) {
                    expression = Some(self.parse_reduce_by_suffix(completed));
                    continue;
                }
                let member = self.precede(completed);
                self.bump();
                if self.eat(TokenKind::TupleIndex) {
                    expression = Some(self.complete(member, SyntaxKind::TupleProjectionExpr));
                } else if self.at(TokenKind::Identifier) || self.at(TokenKind::BangIdentifier) {
                    self.bump();
                    expression = Some(self.complete(member, SyntaxKind::MemberExpr));
                } else {
                    self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
                    expression = Some(self.complete(member, SyntaxKind::MemberExpr));
                }
                continue;
            }

            if self.at(TokenKind::ColonColon) && self.nth_at(1, TokenKind::LeftBracket) {
                let call = self.precede(completed);
                self.bump();
                self.parse_type_argument_list();
                self.parse_call_suffix();
                expression = Some(self.complete(call, SyntaxKind::CallExpr));
                continue;
            }
            if self.at(TokenKind::LeftBracket) {
                let call = self.precede(completed);
                self.parse_dtype_bracket();
                self.parse_call_suffix();
                expression = Some(self.complete(call, SyntaxKind::DTypeBracketCallExpr));
                continue;
            }
            if self.at(TokenKind::LeftParen) {
                let call = self.precede(completed);
                self.parse_call_suffix();
                expression = Some(self.complete(call, SyntaxKind::CallExpr));
                continue;
            }
            if self.at(TokenKind::LeftBrace) && allow_outer_brace_suffix {
                let call = self.precede(completed);
                self.parse_lambda_expression();
                expression = Some(self.complete(call, SyntaxKind::CallExpr));
                continue;
            }
            if self.at(TokenKind::Question) {
                let try_expression = self.precede(completed);
                self.bump();
                expression = Some(self.complete(try_expression, SyntaxKind::TryExpr));
                continue;
            }

            expression = Some(completed);
            break;
        }
        expression
    }

    fn parse_primary(&mut self, allow_outer_brace_suffix: bool) -> Option<CompletedMarker> {
        if self.at_tensor_schema_start() {
            return self.parse_tensor_schema();
        }
        match self.current_kind() {
            Some(
                TokenKind::Integer
                | TokenKind::Float
                | TokenKind::String
                | TokenKind::FormatString
                | TokenKind::TrueKw
                | TokenKind::FalseKw
                | TokenKind::AxisName
                | TokenKind::AxisIndex,
            ) => {
                let literal = self.start();
                self.bump();
                Some(self.complete(literal, SyntaxKind::LiteralExpr))
            }
            Some(TokenKind::Identifier) => self.parse_name_primary(allow_outer_brace_suffix),
            Some(TokenKind::BangIdentifier) => {
                let name = self.start();
                self.bump();
                Some(self.complete(name, SyntaxKind::NameExpr))
            }
            Some(TokenKind::Underscore) => {
                if let Some(seen) = &mut self.placeholder_hole {
                    *seen = true;
                }
                let hole = self.start();
                self.bump();
                Some(self.complete(hole, SyntaxKind::HoleExpr))
            }
            Some(TokenKind::LeftParen) => self.parse_parenthesized_expression(),
            Some(TokenKind::LeftBracket) => self.parse_bracket_collection_expression(),
            Some(TokenKind::LeftBrace) => {
                if self.nth_at(1, TokenKind::LogicalOr) || self.nth_at(1, TokenKind::Pipe) {
                    self.parse_lambda_expression()
                } else {
                    self.parse_bare_brace_expression()
                }
            }
            Some(TokenKind::IfKw) => self.parse_if_expression(),
            Some(TokenKind::MatchKw) => self.parse_match_expression(),
            Some(TokenKind::ResultKw) => self.parse_keyword_block(SyntaxKind::ResultExpr),
            Some(TokenKind::CheckedKw) => self.parse_keyword_block(SyntaxKind::CheckedExpr),
            Some(TokenKind::UnsafeKw) => self.parse_keyword_block(SyntaxKind::UnsafeExpr),
            _ => {
                self.error_current(SyntaxDiagnosticKind::ParseExpectedExpression);
                None
            }
        }
    }

    fn parse_name_primary(&mut self, allow_outer_brace_suffix: bool) -> Option<CompletedMarker> {
        let qualified =
            self.nth_at(1, TokenKind::ColonColon) && self.nth_at(2, TokenKind::Identifier);
        let name = self.start();
        self.parse_path(SyntaxKind::Path);
        let mut completed = self.complete(name, SyntaxKind::NameExpr);

        if self.at(TokenKind::LeftBracket) && self.bracket_is_catalog_owner_call() {
            let call = self.precede(completed);
            self.parse_type_argument_list();
            self.expect(TokenKind::Dot);
            if self.at(TokenKind::Identifier) || self.at(TokenKind::BangIdentifier) {
                self.bump();
            } else {
                self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
            }
            self.parse_call_suffix();
            return Some(self.complete(call, SyntaxKind::CallExpr));
        }

        if self.at(TokenKind::LeftBrace)
            && allow_outer_brace_suffix
            && self.brace_is_named_payload()
        {
            completed = self.parse_braced_literal(completed, qualified);
        } else if qualified && self.at(TokenKind::LeftParen) {
            let constructor = self.precede(completed);
            self.bump();
            let values = self.start();
            self.parse_expression_list_until(TokenKind::RightParen);
            self.complete(values, SyntaxKind::ExpressionList);
            self.expect_closing(TokenKind::RightParen);
            completed = self.complete(constructor, SyntaxKind::VariantConstructorExpr);
        } else if qualified {
            let constructor = self.precede(completed);
            completed = self.complete(constructor, SyntaxKind::VariantConstructorExpr);
        }
        Some(completed)
    }

    fn parse_parenthesized_expression(&mut self) -> Option<CompletedMarker> {
        let parenthesized = self.start();
        let tuple = self.parentheses_contain_comma(self.logical_position());
        self.bump();
        self.skip_newlines();
        if self.eat(TokenKind::RightParen) {
            return Some(self.complete(parenthesized, SyntaxKind::UnitExpr));
        }
        self.parse_expression(!tuple, true);
        self.skip_newlines();
        if tuple || self.at(TokenKind::Comma) {
            self.expect(TokenKind::Comma);
            self.skip_newlines();
            while !self.at(TokenKind::RightParen) && !self.at_eof() && !self.halted() {
                self.parse_expression(false, true);
                self.skip_newlines();
                if !self.eat(TokenKind::Comma) {
                    break;
                }
                self.skip_newlines();
            }
            if tuple && !self.at(TokenKind::RightParen) && !self.halted() {
                self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
            }
            self.expect_closing(TokenKind::RightParen);
            Some(self.complete(parenthesized, SyntaxKind::TupleExpr))
        } else {
            self.expect_closing(TokenKind::RightParen);
            Some(self.complete(parenthesized, SyntaxKind::GroupExpr))
        }
    }

    fn parentheses_contain_comma(&mut self, opening: usize) -> bool {
        let Some(close) = self.matching_delimiter(opening) else {
            return false;
        };
        let mut position = opening + 1;
        while position < close {
            match self.kind_at_logical(position) {
                Some(TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::LeftBrace) => {
                    let Some(nested_close) = self.matching_delimiter(position) else {
                        return false;
                    };
                    position = nested_close + 1;
                }
                Some(TokenKind::Comma) => return true,
                Some(_) => position += 1,
                None => return false,
            }
        }
        false
    }

    fn parse_bracket_collection_expression(&mut self) -> Option<CompletedMarker> {
        let collection = self.start();
        self.bump();
        self.skip_newlines();

        if self.eat(TokenKind::Colon) {
            self.skip_newlines();
            self.expect_closing(TokenKind::RightBracket);
            return Some(self.complete(collection, SyntaxKind::MapExpr));
        }

        let items = self.start();
        if self.at(TokenKind::RightBracket) {
            self.complete(items, SyntaxKind::ExpressionList);
            self.bump();
            return Some(self.complete(collection, SyntaxKind::ArrayExpr));
        }

        let Some(first) = self.parse_expression(false, true) else {
            self.complete(items, SyntaxKind::ExpressionList);
            self.expect_closing(TokenKind::RightBracket);
            return Some(self.complete(collection, SyntaxKind::ArrayExpr));
        };

        if self.eat(TokenKind::Colon) {
            self.skip_newlines();
            let entry = self.precede(first);
            self.parse_expression(false, true);
            self.complete(entry, SyntaxKind::MapEntry);
            self.skip_newlines();

            while self.eat(TokenKind::Comma) {
                self.skip_newlines();
                if self.at(TokenKind::RightBracket) {
                    break;
                }
                let entry = self.start();
                self.parse_expression(false, true);
                self.expect(TokenKind::Colon);
                self.skip_newlines();
                self.parse_expression(false, true);
                self.complete(entry, SyntaxKind::MapEntry);
                self.skip_newlines();
            }
            self.complete(items, SyntaxKind::MapEntryList);
            if !self.at(TokenKind::RightBracket) && !self.halted() {
                self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
            }
            self.expect_closing(TokenKind::RightBracket);
            return Some(self.complete(collection, SyntaxKind::MapExpr));
        }

        self.skip_newlines();
        while self.eat(TokenKind::Comma) {
            self.skip_newlines();
            if self.at(TokenKind::RightBracket) {
                break;
            }
            self.parse_expression(false, true);
            self.skip_newlines();
        }
        self.complete(items, SyntaxKind::ExpressionList);
        if !self.at(TokenKind::RightBracket) && !self.halted() {
            self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
        }
        self.expect_closing(TokenKind::RightBracket);
        Some(self.complete(collection, SyntaxKind::ArrayExpr))
    }

    fn parse_braced_literal(&mut self, owner: CompletedMarker, qualified: bool) -> CompletedMarker {
        let literal = self.precede(owner);
        self.bump();
        let fields = self.start();
        self.skip_newlines();
        while !self.at(TokenKind::RightBrace) && !self.at_eof() && !self.halted() {
            let field = self.start();
            self.expect_identifier();
            self.expect(TokenKind::Colon);
            self.parse_expression(false, true);
            self.complete(field, SyntaxKind::FieldInitializer);
            self.skip_newlines();
            if !self.eat(TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }
        self.complete(fields, SyntaxKind::FieldInitializerList);
        self.expect_closing(TokenKind::RightBrace);
        self.complete(
            literal,
            if qualified {
                SyntaxKind::VariantConstructorExpr
            } else {
                SyntaxKind::StructLiteralExpr
            },
        )
    }

    fn parse_call_suffix(&mut self) {
        self.expect(TokenKind::LeftParen);
        let arguments = self.start();
        self.parse_call_arguments_until_close();
        self.complete(arguments, SyntaxKind::ArgumentList);
        self.expect_closing(TokenKind::RightParen);
    }

    pub(super) fn parse_call_arguments_until_close(&mut self) {
        self.skip_newlines();
        while !self.at(TokenKind::RightParen) && !self.at_eof() && !self.halted() {
            if self.at(TokenKind::Identifier) && self.nth_at(1, TokenKind::Equal) {
                let named = self.start();
                self.bump();
                self.bump();
                self.parse_expression(false, true);
                self.complete(named, SyntaxKind::NamedArgument);
            } else {
                self.parse_expression(false, true);
            }
            self.skip_newlines();
            if !self.eat(TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }
        if !self.at(TokenKind::RightParen) && !self.halted() {
            self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
        }
    }

    fn parse_expression_list_until(&mut self, closing: TokenKind) {
        if self.at(closing) {
            return;
        }
        while !self.at(closing) && !self.at_eof() && !self.halted() {
            self.parse_expression(false, true);
            self.skip_newlines();
            if !self.eat(TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }
        if !self.at(closing) && !self.halted() {
            self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
        }
    }

    fn parse_dtype_bracket(&mut self) {
        let arguments = self.start();
        self.expect(TokenKind::LeftBracket);
        self.skip_newlines();
        self.parse_type();
        self.skip_newlines();
        if self.at(TokenKind::Comma) {
            self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
        }
        self.expect_closing(TokenKind::RightBracket);
        self.complete(arguments, SyntaxKind::TypeArgumentList);
    }

    fn parse_keyword_block(&mut self, kind: SyntaxKind) -> Option<CompletedMarker> {
        let expression = self.start();
        self.bump();
        self.parse_block_expression();
        Some(self.complete(expression, kind))
    }

    fn next_binary_operator(&self, grouped: bool) -> Option<(usize, u8, bool)> {
        let mut distance = 0;
        if grouped {
            while self.nth_at(distance, TokenKind::Newline) {
                distance += 1;
            }
        }
        let (precedence, is_range) = match self.nth_kind(distance)? {
            TokenKind::LogicalOr => (1, false),
            TokenKind::LogicalAnd => (2, false),
            TokenKind::EqualEqual
            | TokenKind::NotEqual
            | TokenKind::Less
            | TokenKind::LessEqual
            | TokenKind::Greater
            | TokenKind::GreaterEqual => (3, false),
            TokenKind::Range | TokenKind::RangeInclusive => (4, true),
            TokenKind::Plus | TokenKind::Minus => (5, false),
            TokenKind::Star
            | TokenKind::Slash
            | TokenKind::FractionalDivide
            | TokenKind::Percent => (6, false),
            _ => return None,
        };
        Some((distance, precedence, is_range))
    }

    fn newlines_before_dot(&self) -> usize {
        let mut distance = 0;
        while self.nth_at(distance, TokenKind::Newline) {
            distance += 1;
        }
        if distance > 0 && self.nth_at(distance, TokenKind::Dot) {
            distance
        } else {
            0
        }
    }

    fn expression_can_start(&self) -> bool {
        matches!(
            self.current_kind(),
            Some(
                TokenKind::Identifier
                    | TokenKind::BangIdentifier
                    | TokenKind::Underscore
                    | TokenKind::Integer
                    | TokenKind::Float
                    | TokenKind::String
                    | TokenKind::FormatString
                    | TokenKind::AxisName
                    | TokenKind::AxisIndex
                    | TokenKind::TrueKw
                    | TokenKind::FalseKw
                    | TokenKind::LeftParen
                    | TokenKind::LeftBracket
                    | TokenKind::LeftBrace
                    | TokenKind::IfKw
                    | TokenKind::MatchKw
                    | TokenKind::ResultKw
                    | TokenKind::CheckedKw
                    | TokenKind::UnsafeKw
                    | TokenKind::OwnKw
                    | TokenKind::MoveKw
                    | TokenKind::DropOwnKw
                    | TokenKind::Ampersand
                    | TokenKind::Bang
                    | TokenKind::Minus
                    | TokenKind::Star
            )
        )
    }

    fn bracket_is_catalog_owner_call(&mut self) -> bool {
        let opening = self.logical_position();
        let Some(close) = self.matching_delimiter(opening) else {
            return false;
        };
        self.kind_at_logical(close + 1) == Some(TokenKind::Dot)
            && matches!(
                self.kind_at_logical(close + 2),
                Some(TokenKind::Identifier | TokenKind::BangIdentifier)
            )
            && self.kind_at_logical(close + 3) == Some(TokenKind::LeftParen)
    }
}

impl Parser<'_> {
    pub(super) fn parse_block_expression(&mut self) -> Option<CompletedMarker> {
        let block = self.start();
        if !self.expect(TokenKind::LeftBrace) {
            return Some(self.complete(block, SyntaxKind::BlockExpr));
        }
        self.skip_newlines();
        self.parse_block_statements();
        self.expect_closing(TokenKind::RightBrace);
        Some(self.complete(block, SyntaxKind::BlockExpr))
    }

    fn parse_bare_brace_expression(&mut self) -> Option<CompletedMarker> {
        let brace = self.start();
        self.expect(TokenKind::LeftBrace);
        self.skip_newlines();

        if self.expression_can_start() {
            let first = self.start();
            let previous_hole = self.placeholder_hole.replace(false);
            let expression = self.parse_expression(false, true);
            let has_hole = self.placeholder_hole.unwrap_or(false);
            self.placeholder_hole = previous_hole;

            if self.eat(TokenKind::Equal) {
                self.parse_expression(false, true);
                self.line_end();
                self.complete(first, SyntaxKind::AssignStmt);
            } else {
                self.line_end();
                if expression.is_some()
                    && has_hole
                    && self.at(TokenKind::RightBrace)
                    && !self.halted()
                {
                    self.complete(first, SyntaxKind::LambdaBody);
                    self.expect_closing(TokenKind::RightBrace);
                    return Some(self.complete(brace, SyntaxKind::LambdaExpr));
                }
                self.complete(first, SyntaxKind::ExprStmt);
            }
        }

        self.parse_block_statements();
        self.expect_closing(TokenKind::RightBrace);
        Some(self.complete(brace, SyntaxKind::BlockExpr))
    }

    fn parse_block_statements(&mut self) {
        while !self.at(TokenKind::RightBrace) && !self.at_eof() && !self.halted() {
            if self.at(TokenKind::YieldKw) {
                self.parse_yield_statement();
                self.skip_newlines();
                if !self.at(TokenKind::RightBrace) && !self.halted() {
                    self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
                }
                break;
            }
            self.parse_statement();
            self.skip_newlines();
        }
    }

    pub(super) fn parse_statement(&mut self) -> Option<CompletedMarker> {
        match self.current_kind() {
            Some(TokenKind::LetKw | TokenKind::VarKw) => self.parse_let_statement(),
            Some(TokenKind::WhileKw) => self.parse_while_statement(),
            Some(TokenKind::ForKw) => self.parse_for_statement(),
            Some(TokenKind::BreakKw) => self.parse_simple_statement(SyntaxKind::BreakStmt),
            Some(TokenKind::ContinueKw) => self.parse_simple_statement(SyntaxKind::ContinueStmt),
            Some(TokenKind::ReturnKw) => self.parse_return_statement(),
            _ if self.expression_can_start() => self.parse_assignment_or_expression_statement(),
            _ => {
                self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
                None
            }
        }
    }

    fn parse_let_statement(&mut self) -> Option<CompletedMarker> {
        let statement = self.start();
        self.bump();
        self.eat(TokenKind::MutKw);
        self.parse_pattern();
        if self.eat(TokenKind::Colon) {
            self.parse_type();
        }
        if self.eat(TokenKind::Equal) {
            self.parse_expression(false, true);
        }
        self.line_end();
        Some(self.complete(statement, SyntaxKind::LetStmt))
    }

    fn parse_assignment_or_expression_statement(&mut self) -> Option<CompletedMarker> {
        let expression = self.parse_expression(false, true)?;
        if self.at(TokenKind::Equal) {
            let assignment = self.precede(expression);
            self.bump();
            self.parse_expression(false, true);
            self.line_end();
            Some(self.complete(assignment, SyntaxKind::AssignStmt))
        } else {
            let statement = self.precede(expression);
            self.line_end();
            Some(self.complete(statement, SyntaxKind::ExprStmt))
        }
    }

    fn parse_while_statement(&mut self) -> Option<CompletedMarker> {
        let statement = self.start();
        self.bump();
        self.parse_control_head_and_block();
        Some(self.complete(statement, SyntaxKind::WhileStmt))
    }

    fn parse_for_statement(&mut self) -> Option<CompletedMarker> {
        let statement = self.start();
        self.bump();
        self.expect_identifier();
        self.expect(TokenKind::InKw);
        self.parse_control_head_and_block();
        Some(self.complete(statement, SyntaxKind::ForStmt))
    }

    fn parse_simple_statement(&mut self, kind: SyntaxKind) -> Option<CompletedMarker> {
        let statement = self.start();
        self.bump();
        self.line_end();
        Some(self.complete(statement, kind))
    }

    fn parse_return_statement(&mut self) -> Option<CompletedMarker> {
        let statement = self.start();
        self.bump();
        if !self.at(TokenKind::Newline) && !self.at(TokenKind::RightBrace) && !self.at_eof() {
            self.parse_expression(false, true);
        }
        self.line_end();
        Some(self.complete(statement, SyntaxKind::ReturnStmt))
    }

    fn parse_yield_statement(&mut self) -> Option<CompletedMarker> {
        let statement = self.start();
        self.bump();
        if self.expression_can_start() {
            self.parse_expression(false, true);
        } else {
            self.error_current(SyntaxDiagnosticKind::ParseExpectedExpression);
        }
        self.line_end();
        Some(self.complete(statement, SyntaxKind::YieldStmt))
    }

    fn parse_if_expression(&mut self) -> Option<CompletedMarker> {
        let conditional = self.start();
        self.bump();
        self.parse_control_head_and_block();
        if self.eat(TokenKind::ElseKw) {
            if self.at(TokenKind::IfKw) {
                if self.enter_recursive() {
                    self.parse_if_expression();
                    self.leave_recursive();
                }
            } else {
                self.parse_block_expression();
                if self.eat(TokenKind::UnknownKw) {
                    self.parse_block_expression();
                }
            }
        }
        Some(self.complete(conditional, SyntaxKind::IfExpr))
    }

    fn parse_match_expression(&mut self) -> Option<CompletedMarker> {
        let match_expression = self.start();
        self.bump();
        self.parse_expression(false, false);
        if self.at(TokenKind::LeftBrace) && self.control_brace_is_restricted_suffix() {
            self.error_current(SyntaxDiagnosticKind::ParseControlHeadBraceRequiresGrouping);
        }
        self.expect(TokenKind::LeftBrace);
        self.skip_newlines();
        while !self.at(TokenKind::RightBrace) && !self.at_eof() && !self.halted() {
            self.parse_match_arm();
            self.skip_newlines();
        }
        self.expect_closing(TokenKind::RightBrace);
        Some(self.complete(match_expression, SyntaxKind::MatchExpr))
    }

    fn parse_match_arm(&mut self) {
        let arm = self.start();
        self.parse_pattern();
        if self.eat(TokenKind::IfKw) {
            let guard = self.start();
            self.parse_expression(false, true);
            self.complete(guard, SyntaxKind::MatchGuard);
        }
        self.expect(TokenKind::FatArrow);
        self.skip_newlines();
        self.parse_expression(false, true);
        if self.eat(TokenKind::Comma) {
            self.skip_newlines();
        } else {
            self.line_end();
        }
        self.complete(arm, SyntaxKind::MatchArm);
    }

    fn parse_control_head_and_block(&mut self) {
        self.parse_expression(false, false);
        if self.at(TokenKind::LeftBrace) && self.control_brace_is_restricted_suffix() {
            self.error_current(SyntaxDiagnosticKind::ParseControlHeadBraceRequiresGrouping);
        }
        self.parse_block_expression();
    }

    #[cfg(test)]
    pub(super) fn parse_control_construct(&mut self) -> Option<CompletedMarker> {
        let control = self.start();
        let kind = match self.current_kind() {
            Some(TokenKind::IfKw) => SyntaxKind::IfExpr,
            Some(TokenKind::WhileKw) => SyntaxKind::WhileStmt,
            Some(TokenKind::ForKw) => SyntaxKind::ForStmt,
            Some(TokenKind::MatchKw) => SyntaxKind::MatchExpr,
            _ => {
                self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
                return None;
            }
        };
        self.bump();
        if kind == SyntaxKind::ForStmt {
            self.expect_identifier();
            self.expect(TokenKind::InKw);
        }
        self.parse_control_head_and_block();
        Some(self.complete(control, kind))
    }

    pub(super) fn parse_lambda_expression(&mut self) -> Option<CompletedMarker> {
        let previous_hole = self.placeholder_hole.take();
        let lambda = self.start();
        self.expect(TokenKind::LeftBrace);
        if self.eat(TokenKind::LogicalOr) {
            // The lexer owns `||`; only this position reinterprets it as no parameters.
        } else if self.eat(TokenKind::Pipe) {
            let parameters = self.start();
            if !self.eat(TokenKind::Pipe) {
                while !self.at(TokenKind::Pipe) && !self.at_eof() && !self.halted() {
                    let parameter = self.start();
                    if !self.eat(TokenKind::Underscore) {
                        self.expect_identifier();
                    }
                    if self.eat(TokenKind::Colon) {
                        self.parse_type();
                    }
                    self.complete(parameter, SyntaxKind::Parameter);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::Pipe);
            }
            self.complete(parameters, SyntaxKind::LambdaParameterList);
        }

        let body = self.start();
        self.skip_newlines();
        while !self.at(TokenKind::RightBrace) && !self.at_eof() && !self.halted() {
            if matches!(
                self.current_kind(),
                Some(
                    TokenKind::LetKw
                        | TokenKind::VarKw
                        | TokenKind::WhileKw
                        | TokenKind::ForKw
                        | TokenKind::BreakKw
                        | TokenKind::ContinueKw
                        | TokenKind::ReturnKw
                )
            ) {
                self.parse_statement();
                self.skip_newlines();
                continue;
            }
            if !self.expression_can_start() {
                self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
                break;
            }

            let expression = self.parse_expression(false, true);
            if self.at(TokenKind::Equal) {
                if let Some(expression) = expression {
                    let assignment = self.precede(expression);
                    self.bump();
                    self.parse_expression(false, true);
                    self.line_end();
                    self.complete(assignment, SyntaxKind::AssignStmt);
                }
                self.skip_newlines();
            } else if self.at(TokenKind::Newline) {
                if let Some(expression) = expression {
                    self.skip_newlines();
                    if !self.at(TokenKind::RightBrace) {
                        let statement = self.precede(expression);
                        self.complete(statement, SyntaxKind::ExprStmt);
                    }
                }
            } else if !self.at(TokenKind::RightBrace) {
                self.error_current(SyntaxDiagnosticKind::ParseUnexpectedToken);
            }
        }
        self.complete(body, SyntaxKind::LambdaBody);
        self.expect_closing(TokenKind::RightBrace);
        self.placeholder_hole = previous_hole;
        Some(self.complete(lambda, SyntaxKind::LambdaExpr))
    }

    fn brace_is_named_payload(&mut self) -> bool {
        let opening = self.logical_position();
        if self.nth_at(1, TokenKind::LogicalOr) || self.nth_at(1, TokenKind::Pipe) {
            return false;
        }
        let Some(close) = self.matching_delimiter(opening) else {
            return true;
        };
        let mut first = opening + 1;
        while self.kind_at_logical(first) == Some(TokenKind::Newline) {
            first += 1;
        }
        first == close || self.brace_has_top_level_colon(opening, close)
    }

    fn brace_has_possible_placeholder_token(&self, opening: usize, close: usize) -> bool {
        (opening + 1..close).any(|position| {
            self.kind_at_logical(position) == Some(TokenKind::Underscore)
                && self.kind_at_logical(position + 1) != Some(TokenKind::Equal)
        })
    }

    fn control_brace_is_restricted_suffix(&mut self) -> bool {
        let opening = self.logical_position();
        let Some(close) = self.matching_delimiter(opening) else {
            return false;
        };

        let mut first = opening + 1;
        while self.kind_at_logical(first) == Some(TokenKind::Newline) {
            first += 1;
        }
        let explicit_closure = matches!(
            self.kind_at_logical(first),
            Some(TokenKind::Pipe | TokenKind::LogicalOr)
        );
        let payload = first == close || self.brace_has_top_level_colon(opening, close);
        // This balanced-token probe selects a diagnostic only, never an AST kind.
        if !explicit_closure
            && !payload
            && !self.brace_has_possible_placeholder_token(opening, close)
        {
            return false;
        }

        let Some(body) = self.next_outer_brace(close + 1) else {
            return false;
        };
        if close + 1 == body {
            return true;
        }

        if self.kind_at_logical(body - 1) == Some(TokenKind::Newline) {
            return false;
        }
        let mut first_continuation = close + 1;
        while self.kind_at_logical(first_continuation) == Some(TokenKind::Newline) {
            first_continuation += 1;
        }
        let Some(kind) = self.kind_at_logical(first_continuation) else {
            return false;
        };
        if first_continuation > close + 1 {
            return kind == TokenKind::Dot;
        }
        matches!(
            kind,
            TokenKind::Dot
                | TokenKind::LeftParen
                | TokenKind::LeftBracket
                | TokenKind::ColonColon
                | TokenKind::Question
                | TokenKind::LogicalOr
                | TokenKind::LogicalAnd
                | TokenKind::EqualEqual
                | TokenKind::NotEqual
                | TokenKind::Less
                | TokenKind::LessEqual
                | TokenKind::Greater
                | TokenKind::GreaterEqual
                | TokenKind::Range
                | TokenKind::RangeInclusive
                | TokenKind::Plus
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Slash
                | TokenKind::FractionalDivide
                | TokenKind::Percent
        )
    }

    fn brace_has_top_level_colon(&mut self, opening: usize, close: usize) -> bool {
        let mut position = opening + 1;
        while position < close {
            match self.kind_at_logical(position) {
                Some(TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::LeftBrace) => {
                    let Some(nested_close) = self.matching_delimiter(position) else {
                        return false;
                    };
                    position = nested_close + 1;
                }
                Some(TokenKind::Colon) => return true,
                Some(_) => position += 1,
                None => return false,
            }
        }
        false
    }

    fn next_outer_brace(&mut self, mut position: usize) -> Option<usize> {
        while let Some(kind) = self.kind_at_logical(position) {
            match kind {
                TokenKind::LeftBrace => return Some(position),
                TokenKind::LeftParen | TokenKind::LeftBracket => {
                    position = self.matching_delimiter(position)? + 1;
                }
                TokenKind::RightParen | TokenKind::RightBracket | TokenKind::RightBrace => {
                    return None;
                }
                _ => position += 1,
            }
        }
        None
    }
}
