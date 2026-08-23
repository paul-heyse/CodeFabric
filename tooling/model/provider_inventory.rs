//! Library-native provider inventory probe for the model compiler.
//!
//! This binary has no repository write capability. Tree-sitter inventories come from
//! `Language`; Ruff 0.0.7 has no public enum iterator, so one macro expands a declaration-order
//! inventory and an exhaustive match. An upstream Ruff variant therefore makes this target fail
//! to compile until the new variant is classified.

use ruff_python_ast::NodeKind;
use ruff_python_ast::token::TokenKind;
use serde::Serialize;

#[derive(Serialize)]
struct Probe {
    schema_version: u8,
    tree_sitter: Vec<TreeSitterInventory>,
    ruff: RuffInventory,
}

#[derive(Serialize)]
struct TreeSitterInventory {
    catalog_id: &'static str,
    provider_version: &'static str,
    language: &'static str,
    grammar_abi: usize,
    node_types_source: &'static str,
    raw_kinds: Vec<TreeSitterKind>,
    fields: Vec<TreeSitterField>,
}

#[derive(Serialize)]
struct TreeSitterKind {
    raw_kind_id: u16,
    raw_name: String,
    named: bool,
    visible: bool,
    supertype: bool,
    subtypes: Vec<String>,
}

#[derive(Serialize)]
struct TreeSitterField {
    field_id: u16,
    field_name: String,
}

#[derive(Serialize)]
struct RuffInventory {
    catalog_id: &'static str,
    provider_version: &'static str,
    language: &'static str,
    node_kinds: Vec<RuffKind>,
    token_kinds: Vec<RuffKind>,
}

#[derive(Serialize)]
struct RuffKind {
    raw_kind_id: u16,
    raw_name: &'static str,
}

macro_rules! exhaustive_inventory {
    ($kind:path, $name:ident, [$($variant:ident),+ $(,)?]) => {
        const $name: &[$kind] = &[$(<$kind>::$variant),+];

        const fn variant_name(value: $kind) -> &'static str {
            match value {
                $(<$kind>::$variant => stringify!($variant)),+
            }
        }
    };
}

exhaustive_inventory!(
    NodeKind,
    RUFF_NODE_KINDS,
    [
        ModModule,
        ModExpression,
        StmtFunctionDef,
        StmtClassDef,
        StmtReturn,
        StmtDelete,
        StmtTypeAlias,
        StmtAssign,
        StmtAugAssign,
        StmtAnnAssign,
        StmtFor,
        StmtWhile,
        StmtIf,
        StmtWith,
        StmtMatch,
        StmtRaise,
        StmtTry,
        StmtAssert,
        StmtImport,
        StmtImportFrom,
        StmtGlobal,
        StmtNonlocal,
        StmtExpr,
        StmtPass,
        StmtBreak,
        StmtContinue,
        StmtIpyEscapeCommand,
        ExprBoolOp,
        ExprNamed,
        ExprBinOp,
        ExprUnaryOp,
        ExprLambda,
        ExprIf,
        ExprDict,
        ExprSet,
        ExprListComp,
        ExprSetComp,
        ExprDictComp,
        ExprGenerator,
        ExprAwait,
        ExprYield,
        ExprYieldFrom,
        ExprCompare,
        ExprCall,
        ExprFString,
        ExprTString,
        ExprStringLiteral,
        ExprBytesLiteral,
        ExprNumberLiteral,
        ExprBooleanLiteral,
        ExprNoneLiteral,
        ExprEllipsisLiteral,
        ExprAttribute,
        ExprSubscript,
        ExprStarred,
        ExprName,
        ExprList,
        ExprTuple,
        ExprSlice,
        ExprIpyEscapeCommand,
        ExceptHandlerExceptHandler,
        InterpolatedElement,
        InterpolatedStringLiteralElement,
        PatternMatchValue,
        PatternMatchSingleton,
        PatternMatchSequence,
        PatternMatchMapping,
        PatternMatchClass,
        PatternMatchStar,
        PatternMatchAs,
        PatternMatchOr,
        TypeParamTypeVar,
        TypeParamTypeVarTuple,
        TypeParamParamSpec,
        InterpolatedStringFormatSpec,
        PatternArguments,
        PatternKeyword,
        Comprehension,
        Arguments,
        Parameters,
        Parameter,
        ParameterWithDefault,
        Keyword,
        Alias,
        WithItem,
        MatchCase,
        Decorator,
        ElifElseClause,
        TypeParams,
        FString,
        TString,
        StringLiteral,
        BytesLiteral,
        Identifier,
    ]
);

mod tokens {
    use super::TokenKind;

    exhaustive_inventory!(
        TokenKind,
        RUFF_TOKEN_KINDS,
        [
            Name,
            Int,
            Float,
            Complex,
            String,
            FStringStart,
            FStringMiddle,
            FStringEnd,
            TStringStart,
            TStringMiddle,
            TStringEnd,
            IpyEscapeCommand,
            Comment,
            Newline,
            NonLogicalNewline,
            Indent,
            Dedent,
            EndOfFile,
            Question,
            Exclamation,
            Lpar,
            Rpar,
            Lsqb,
            Rsqb,
            Colon,
            Comma,
            Semi,
            Plus,
            Minus,
            Star,
            Slash,
            Vbar,
            Amper,
            Less,
            Greater,
            Equal,
            Dot,
            Percent,
            Lbrace,
            Rbrace,
            EqEqual,
            NotEqual,
            LessEqual,
            GreaterEqual,
            Tilde,
            CircumFlex,
            LeftShift,
            RightShift,
            DoubleStar,
            DoubleStarEqual,
            PlusEqual,
            MinusEqual,
            StarEqual,
            SlashEqual,
            PercentEqual,
            AmperEqual,
            VbarEqual,
            CircumflexEqual,
            LeftShiftEqual,
            RightShiftEqual,
            DoubleSlash,
            DoubleSlashEqual,
            ColonEqual,
            At,
            AtEqual,
            Rarrow,
            Ellipsis,
            And,
            As,
            Assert,
            Async,
            Await,
            Break,
            Class,
            Continue,
            Def,
            Del,
            Elif,
            Else,
            Except,
            False,
            Finally,
            For,
            From,
            Global,
            If,
            Import,
            In,
            Is,
            Lambda,
            None,
            Nonlocal,
            Not,
            Or,
            Pass,
            Raise,
            Return,
            True,
            Try,
            While,
            With,
            Yield,
            Case,
            Lazy,
            Match,
            Type,
            Unknown,
        ]
    );

    pub(super) fn inventory() -> Vec<super::RuffKind> {
        RUFF_TOKEN_KINDS
            .iter()
            .copied()
            .enumerate()
            .map(|(id, kind)| super::RuffKind {
                raw_kind_id: u16::try_from(id).expect("Ruff token inventory fits u16"),
                raw_name: variant_name(kind),
            })
            .collect()
    }
}

fn tree_sitter_inventory(
    catalog_id: &'static str,
    provider_version: &'static str,
    language_name: &'static str,
    language: &tree_sitter::Language,
    node_types: &'static str,
) -> TreeSitterInventory {
    assert!(u16::try_from(language.node_kind_count()).is_ok());
    assert!(u16::try_from(language.field_count()).is_ok());
    let raw_kinds = (0..language.node_kind_count())
        .map(|id| u16::try_from(id).expect("bounded above"))
        .chain([u16::MAX - 1, u16::MAX])
        .filter_map(|id| {
            let raw_kind_id = id;
            language
                .node_kind_for_id(raw_kind_id)
                .map(|raw_name| TreeSitterKind {
                    raw_kind_id,
                    raw_name: raw_name.to_owned(),
                    named: language.node_kind_is_named(raw_kind_id),
                    visible: language.node_kind_is_visible(raw_kind_id),
                    supertype: language.node_kind_is_supertype(raw_kind_id),
                    subtypes: language
                        .subtypes_for_supertype(raw_kind_id)
                        .iter()
                        .filter_map(|id| language.node_kind_for_id(*id))
                        .map(str::to_owned)
                        .collect(),
                })
        })
        .collect();
    let fields = (1..=language.field_count())
        .filter_map(|id| {
            let field_id = u16::try_from(id).expect("bounded above");
            language
                .field_name_for_id(field_id)
                .map(|field_name| TreeSitterField {
                    field_id,
                    field_name: field_name.to_owned(),
                })
        })
        .collect();
    TreeSitterInventory {
        catalog_id,
        provider_version,
        language: language_name,
        grammar_abi: language.abi_version(),
        node_types_source: node_types,
        raw_kinds,
        fields,
    }
}

fn main() {
    let probe = Probe {
        schema_version: 1,
        tree_sitter: vec![
            tree_sitter_inventory(
                "tree-sitter-python-0-25-0",
                "tree-sitter=0.26.12;tree-sitter-python=0.25.0",
                "python",
                &tree_sitter_python::LANGUAGE.into(),
                tree_sitter_python::NODE_TYPES,
            ),
            tree_sitter_inventory(
                "tree-sitter-rust-0-24-2",
                "tree-sitter=0.26.12;tree-sitter-rust=0.24.2",
                "rust",
                &tree_sitter_rust::LANGUAGE.into(),
                tree_sitter_rust::NODE_TYPES,
            ),
        ],
        ruff: RuffInventory {
            catalog_id: "ruff-python-0-0-7",
            provider_version: "ruff-python-ast=0.0.7;ruff-python-parser=0.0.7;python-target=3.14",
            language: "python",
            node_kinds: RUFF_NODE_KINDS
                .iter()
                .copied()
                .enumerate()
                .map(|(id, kind)| RuffKind {
                    raw_kind_id: u16::try_from(id).expect("Ruff node inventory fits u16"),
                    raw_name: variant_name(kind),
                })
                .collect(),
            token_kinds: tokens::inventory(),
        },
    };
    serde_json::to_writer(std::io::stdout().lock(), &probe).expect("stdout accepts probe JSON");
}
