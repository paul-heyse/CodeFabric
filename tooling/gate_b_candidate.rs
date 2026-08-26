use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use codefabric::gate_b_candidate::{
    check_candidate_bundle, generate_candidate_bundle, verify_candidate_bundle,
    write_candidate_bundle,
};
use codefabric::gate_b_release::{
    AcceptanceAuthorization, accept_candidate, check_released_gate_b, verify_release_chain,
};

fn usage() -> ! {
    eprintln!(
        "usage: codefabric-gate-b-candidate emit <repository-root> <corpus-root> <scratch-root> <output-relative>\n       codefabric-gate-b-candidate check <repository-root> <corpus-root> <scratch-root> <candidate-root>\n       codefabric-gate-b-candidate verify <candidate-root>\n       codefabric-gate-b-candidate accept <repository-root> <candidate-relative> <acceptance-relative> <owner-identity> <provenance>\n       codefabric-gate-b-candidate verify-release <repository-root>\n       codefabric-gate-b-candidate check-release <repository-root> <scratch-root>"
    );
    std::process::exit(2);
}

fn path(argument: Option<String>) -> PathBuf {
    PathBuf::from(argument.unwrap_or_else(|| usage()))
}

fn repository_root(argument: Option<String>) -> Result<PathBuf, std::io::Error> {
    std::fs::canonicalize(path(argument))
}

fn anchored(root: &Path, candidate: PathBuf) -> PathBuf {
    if candidate.is_absolute() {
        candidate
    } else {
        root.join(candidate)
    }
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
            let repository_root = repository_root(arguments.next())?;
            let corpus_root = anchored(&repository_root, path(arguments.next()));
            let scratch_root = anchored(&repository_root, path(arguments.next()));
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
            let repository_root = repository_root(arguments.next())?;
            let corpus_root = anchored(&repository_root, path(arguments.next()));
            let scratch_root = anchored(&repository_root, path(arguments.next()));
            let candidate_root = anchored(&repository_root, path(arguments.next()));
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
        Some("accept") => {
            let repository_root = repository_root(arguments.next())?;
            let candidate_relative = path(arguments.next());
            let acceptance_relative = path(arguments.next());
            let owner_identity = arguments.next().unwrap_or_else(|| usage());
            let acceptance_provenance = arguments.next().unwrap_or_else(|| usage());
            if arguments.next().is_some() {
                usage();
            }
            let accepted_at_unix_seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            accept_candidate(
                &repository_root,
                &candidate_relative,
                &acceptance_relative,
                &AcceptanceAuthorization {
                    owner_identity,
                    acceptance_provenance,
                    accepted_at_unix_seconds,
                    reviewed_candidate: true,
                },
            )?;
            println!("Gate B owner acceptance and immutable corpus release published");
        }
        Some("verify-release") => {
            let repository_root = repository_root(arguments.next())?;
            if arguments.next().is_some() {
                usage();
            }
            verify_release_chain(&repository_root)?;
            println!("Gate B owner acceptance and immutable corpus release are valid");
        }
        Some("check-release") => {
            let repository_root = repository_root(arguments.next())?;
            let scratch_root = anchored(&repository_root, path(arguments.next()));
            if arguments.next().is_some() {
                usage();
            }
            check_released_gate_b(&repository_root, &scratch_root)?;
            println!("Released Gate B corpus is accepted, reproducible, and executable");
        }
        _ => usage(),
    }
    Ok(())
}
