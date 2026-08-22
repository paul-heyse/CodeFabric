use ruff_python_ast::PySourceType;
use ruff_python_index::Indexer;
use ruff_python_parser::parse_unchecked_source;
use ruff_python_trivia::TriviaRanges;
use ruff_source_file::LineIndex;
use ruff_text_size::TextSize;
use tree_sitter::{LANGUAGE_VERSION, MIN_COMPATIBLE_LANGUAGE_VERSION, Parser};

fn parse_with_tree_sitter(language: &tree_sitter::Language, source: &[u8]) -> tree_sitter::Tree {
    let mut parser = Parser::new();
    parser
        .set_language(language)
        .expect("the exact grammar must be compatible with the pinned runtime ABI");
    parser.parse(source, None).expect("parse was not cancelled")
}

#[test]
fn wp27_behavioral_acceptance() {
    let python: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let rust: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();

    let python_tree = parse_with_tree_sitter(&python, b"def answer() -> int:\n    return 42\n");
    let rust_tree = parse_with_tree_sitter(&rust, b"fn answer() -> i32 { 42 }\n");
    assert!(!python_tree.root_node().has_error());
    assert!(!rust_tree.root_node().has_error());

    let parsed = parse_unchecked_source("value = answer()\n", PySourceType::Python);
    assert!(parsed.errors().is_empty());
    assert_eq!(parsed.suite().len(), 1);
    assert!(parsed.tokens().len() > 1);
}

#[test]
fn wp27_structural_acceptance() {
    for language in [
        tree_sitter::Language::from(tree_sitter_python::LANGUAGE),
        tree_sitter::Language::from(tree_sitter_rust::LANGUAGE),
    ] {
        assert_eq!(language.abi_version(), 15);
        assert!(
            (MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION).contains(&language.abi_version())
        );
        assert!(language.node_kind_count() > 0);
        assert!(language.field_count() > 0);
    }

    let source = "# retained trivia\nvalue = (1 + 2)\n";
    let parsed = parse_unchecked_source(source, PySourceType::Python);
    let trivia = TriviaRanges::from(parsed.tokens());
    let indexer = Indexer::from_tokens(parsed.tokens(), source);
    let line_index = LineIndex::from_source_text(source);
    assert_eq!(trivia.comments().len(), 1);
    assert_eq!(indexer.comment_ranges().len(), 1);
    assert_eq!(line_index.line_index(TextSize::new(0)).get(), 1);
}

#[test]
fn wp27_negative_zero_state() {
    let malformed_python = parse_with_tree_sitter(
        &tree_sitter::Language::from(tree_sitter_python::LANGUAGE),
        b"def incomplete(\n",
    );
    assert!(malformed_python.root_node().has_error());

    let recovered = parse_unchecked_source("def incomplete(\n", PySourceType::Python);
    assert!(recovered.has_syntax_errors());
    assert!(!recovered.tokens().is_empty());
}

#[test]
fn wp27_operational_acceptance() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .thread_name(|index| format!("codefabric-provider-{index}"))
        .build()
        .expect("the bounded provider pool must build");
    assert_eq!(pool.current_num_threads(), 1);
    assert_eq!(pool.install(|| (1_u32..=4).sum::<u32>()), 10);
}
