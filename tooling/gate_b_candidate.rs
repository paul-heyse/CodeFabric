use std::path::{Path, PathBuf};

use codefabric::gate_b_candidate::{
    check_candidate_bundle, generate_candidate_bundle, verify_candidate_bundle,
    write_candidate_bundle,
};

fn usage() -> ! {
    eprintln!(
        "usage: codefabric-gate-b-candidate emit <repository-root> <corpus-root> <scratch-root> <output-relative>\n       codefabric-gate-b-candidate check <repository-root> <corpus-root> <scratch-root> <candidate-root>\n       codefabric-gate-b-candidate verify <candidate-root>"
    );
    std::process::exit(2);
}

fn path(argument: Option<String>) -> PathBuf {
    PathBuf::from(argument.unwrap_or_else(|| usage()))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Gate B candidate operation failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    match arguments.next().as_deref() {
        Some("emit") => {
            let repository_root = path(arguments.next());
            let corpus_root = path(arguments.next());
            let scratch_root = path(arguments.next());
            let output = path(arguments.next());
            if arguments.next().is_some() {
                usage();
            }
            let bundle = generate_candidate_bundle(&repository_root, &corpus_root, &scratch_root)?;
            write_candidate_bundle(&repository_root, &output, &bundle)?;
            verify_candidate_bundle(&repository_root.join(&output))?;
            println!("Gate B review candidate emitted at {}", output.display());
        }
        Some("check") => {
            let repository_root = path(arguments.next());
            let corpus_root = path(arguments.next());
            let scratch_root = path(arguments.next());
            let candidate_root = path(arguments.next());
            if arguments.next().is_some() {
                usage();
            }
            check_candidate_bundle(
                &repository_root,
                &corpus_root,
                &scratch_root,
                &candidate_root,
            )?;
            println!("Gate B review candidate is valid and reproducible");
        }
        Some("verify") => {
            let candidate_root = path(arguments.next());
            if arguments.next().is_some() {
                usage();
            }
            verify_candidate_bundle(Path::new(&candidate_root))?;
            println!("Gate B review candidate digest chain is valid");
        }
        _ => usage(),
    }
    Ok(())
}
