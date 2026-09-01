//! Application-owned provider identity and raw-kind normalization policy.
//!
//! Tree-sitter's [`tree_sitter::Language`] is the raw kind and field catalog. Ruff's closed
//! enums are the raw catalog for its exact-pinned release train. This module intentionally keeps
//! only release identity and the small application policy that classifies provider-native kinds;
//! it does not mirror either provider's complete inventory.

use ruff_python_ast::{NodeKind, token::TokenKind};
use tree_sitter::Language;

/// Application disposition for a provider-native kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderRawKindDisposition {
    Normalize,
    Ignore,
    Unsupported,
}

/// Closed grammar identity used to select application normalization policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderGrammarKind {
    Python,
    Rust,
}

/// Minimal application-owned contract for an exact-pinned Tree-sitter grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderGrammarInventory {
    pub grammar: ProviderGrammarKind,
    pub catalog_id: &'static str,
    pub language: &'static str,
    pub provider_version: &'static str,
    pub grammar_abi: usize,
    pub node_types_digest: &'static str,
    pub runtime_inventory_fingerprint: &'static str,
    pub recovery_query_digest: &'static str,
}

/// One live Tree-sitter kind observation plus application normalization policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRawKindEntry {
    pub raw_kind_id: u16,
    pub raw_name: String,
    pub named: bool,
    pub visible: bool,
    pub supertype: bool,
    pub disposition: ProviderRawKindDisposition,
    pub normalized_kind_code: u16,
}

/// One live Ruff AST kind observation plus application normalization policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuffNodeKindEntry {
    pub raw_kind_id: u16,
    pub raw_name: String,
    pub disposition: ProviderRawKindDisposition,
    pub normalized_kind_code: u16,
}

/// One live Ruff token kind observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuffTokenKindEntry {
    pub raw_kind_id: u16,
    pub raw_name: String,
}

/// Minimal application-owned contract for the exact-pinned Ruff frontend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuffPythonInventory {
    pub catalog_id: &'static str,
    pub provider_version: &'static str,
    pub runtime_inventory_fingerprint: &'static str,
}

pub const TREE_SITTER_RECOVERY_QUERY: &str = "(ERROR) @error\n(MISSING) @missing\n";

pub const TREE_SITTER_PYTHON_GRAMMAR: ProviderGrammarInventory = ProviderGrammarInventory {
    grammar: ProviderGrammarKind::Python,
    catalog_id: "tree-sitter-python-0-25-0",
    language: "python",
    provider_version: "tree-sitter=0.26.12;tree-sitter-python=0.25.0",
    grammar_abi: 15,
    node_types_digest: "b3:35f2ec3bdb672cca5ff45c34beb53081f68723304bc5e79fe26b7e1ff4a3beb5",
    runtime_inventory_fingerprint: "b3:2dc18ba76a37182bf98584947382de9ad66e58efd78d33ac857bdb46d041cfce",
    recovery_query_digest: "b3:6c682f80e65948ece8e69833ecb9d779e6c7573067064157e9ec0b3393e39ba3",
};

pub const TREE_SITTER_RUST_GRAMMAR: ProviderGrammarInventory = ProviderGrammarInventory {
    grammar: ProviderGrammarKind::Rust,
    catalog_id: "tree-sitter-rust-0-24-2",
    language: "rust",
    provider_version: "tree-sitter=0.26.12;tree-sitter-rust=0.24.2",
    grammar_abi: 15,
    node_types_digest: "b3:8fd818477e09d44baff5ac7dad2f5586c32d4298fe33a00db372bfa79a428166",
    runtime_inventory_fingerprint: "b3:ce29f0004741844dbc631a161a32bfb9cea7e8af8834f9d0c0189188e9b53c8c",
    recovery_query_digest: "b3:6c682f80e65948ece8e69833ecb9d779e6c7573067064157e9ec0b3393e39ba3",
};

pub const RUFF_PYTHON_FRONTEND: RuffPythonInventory = RuffPythonInventory {
    catalog_id: "ruff-python-0-0-7",
    provider_version: "ruff-python-ast=0.0.7;ruff-python-parser=0.0.7;python-target=3.14",
    runtime_inventory_fingerprint: "b3:22a84ab2f2d25a2e94ceb9639458bc3a8178461d5047152aade06f4d63ebf65d",
};

/// Observe one Tree-sitter kind from the loaded grammar and attach explicit application policy.
#[must_use]
pub fn tree_sitter_raw_kind_entry(
    language: &Language,
    inventory: &ProviderGrammarInventory,
    raw_kind_id: u16,
) -> Option<ProviderRawKindEntry> {
    let raw_name = language.node_kind_for_id(raw_kind_id)?.to_owned();
    let (disposition, normalized_kind_code) =
        tree_sitter_normalization(inventory.grammar, &raw_name);
    Some(ProviderRawKindEntry {
        raw_kind_id,
        raw_name,
        named: language.node_kind_is_named(raw_kind_id),
        visible: language.node_kind_is_visible(raw_kind_id),
        supertype: language.node_kind_is_supertype(raw_kind_id),
        disposition,
        normalized_kind_code,
    })
}

/// Observe one Ruff AST kind from the exact provider enum.
#[must_use]
pub fn ruff_python_node_kind_entry(kind: NodeKind) -> RuffNodeKindEntry {
    RuffNodeKindEntry {
        raw_kind_id: kind as u16,
        raw_name: format!("{kind:?}"),
        disposition: ProviderRawKindDisposition::Normalize,
        normalized_kind_code: ruff_python_normalized_kind_code(kind),
    }
}

/// Observe one Ruff token kind from the exact provider enum.
#[must_use]
pub fn ruff_python_token_kind_entry(kind: TokenKind) -> RuffTokenKindEntry {
    RuffTokenKindEntry {
        raw_kind_id: kind as u16,
        raw_name: format!("{kind:?}"),
    }
}

fn tree_sitter_normalization(
    grammar: ProviderGrammarKind,
    raw_name: &str,
) -> (ProviderRawKindDisposition, u16) {
    use ProviderRawKindDisposition::{Ignore, Normalize, Unsupported};

    match grammar {
        ProviderGrammarKind::Python => match raw_name {
            "comment" => (Ignore, 10),
            "module" => (Normalize, 10),
            "import_statement" | "import_from_statement" => (Normalize, 240),
            "expression_statement" => (Normalize, 20),
            "return_statement" => (Normalize, 200),
            "raise_statement" => (Normalize, 230),
            "if_statement" => (Normalize, 180),
            "for_statement" | "while_statement" => (Normalize, 190),
            "function_definition" | "class_definition" => (Normalize, 50),
            "assignment" => (Normalize, 170),
            "yield" => (Normalize, 210),
            "call" => (Normalize, 160),
            "await" => (Normalize, 220),
            _ => (Unsupported, 10),
        },
        ProviderGrammarKind::Rust => match raw_name {
            "line_comment" | "block_comment" => (Ignore, 10),
            "source_file" => (Normalize, 10),
            "struct_item" | "enum_item" | "function_item" | "trait_item" => (Normalize, 50),
            "use_declaration" => (Normalize, 240),
            "macro_invocation" => (Normalize, 110),
            "assignment_expression" => (Normalize, 170),
            "return_expression" => (Normalize, 200),
            "call_expression" => (Normalize, 160),
            "if_expression" => (Normalize, 180),
            "while_expression" | "loop_expression" | "for_expression" => (Normalize, 190),
            "await_expression" => (Normalize, 220),
            _ => (Unsupported, 10),
        },
    }
}

#[allow(clippy::too_many_lines)] // Exhaustiveness is the deliberate Ruff upgrade sentinel.
const fn ruff_python_normalized_kind_code(kind: NodeKind) -> u16 {
    use NodeKind::*;

    match kind {
        ModModule | ModExpression => 90,
        StmtFunctionDef | StmtClassDef | StmtTypeAlias => 50,
        StmtReturn => 200,
        StmtDelete | StmtWith | StmtTry | StmtAssert | StmtGlobal | StmtNonlocal | StmtExpr
        | StmtPass | StmtBreak | StmtContinue | StmtIpyEscapeCommand => 20,
        StmtAssign | StmtAugAssign | StmtAnnAssign => 170,
        StmtFor | StmtWhile | Comprehension => 190,
        StmtIf | StmtMatch | ExprIf | MatchCase => 180,
        StmtRaise => 230,
        StmtImport | StmtImportFrom | Alias => 240,
        ExprBoolOp | ExprNamed | ExprBinOp | ExprUnaryOp | ExprCompare => 110,
        ExprLambda | ExprDict | ExprSet | ExprListComp | ExprSetComp | ExprDictComp
        | ExprGenerator | ExprStarred | ExprName | ExprList | ExprTuple | ExprSlice
        | ExprIpyEscapeCommand => 30,
        ExprAwait => 220,
        ExprYield | ExprYieldFrom => 210,
        ExprCall => 160,
        ExprFString | ExprTString | ExprStringLiteral | ExprBytesLiteral | ExprNumberLiteral
        | ExprBooleanLiteral | ExprNoneLiteral | ExprEllipsisLiteral | FString | TString
        | StringLiteral | BytesLiteral => 100,
        ExprAttribute => 120,
        ExprSubscript => 140,
        ExceptHandlerExceptHandler
        | InterpolatedElement
        | InterpolatedStringLiteralElement
        | InterpolatedStringFormatSpec
        | WithItem
        | Decorator
        | ElifElseClause
        | Identifier => 10,
        PatternMatchValue
        | PatternMatchSingleton
        | PatternMatchSequence
        | PatternMatchMapping
        | PatternMatchClass
        | PatternMatchStar
        | PatternMatchAs
        | PatternMatchOr
        | PatternArguments
        | PatternKeyword => 40,
        TypeParamTypeVar | TypeParamTypeVarTuple | TypeParamParamSpec | TypeParams => 60,
        Arguments | Keyword => 80,
        Parameters | Parameter | ParameterWithDefault => 70,
    }
}
