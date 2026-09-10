mod interactive;
mod options;
mod partition;
mod pdf;
mod progress;
mod scorecard;
mod wcif;

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::Error as IoError;
use std::{env, process};

use clap::Parser as _;
use interactive::InteractiveError;
use mimalloc::MiMalloc;
use options::{Cli, ResolvedOptions};
use partition::SplitError;
use pdf::PdfGenerationError;
use scorecard::{PlannerError, ScorecardPlanner};
use wcif::{WcifLoadError, WcifLoader};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Top-level application error::Error encompassing all potential failure modes.
#[derive(Debug)]
pub enum AppError {
    Wcif(WcifLoadError),
    Interactive(InteractiveError),
    Planner(PlannerError),
    Split(SplitError),
    CreatePdf {
        path: String,
        source: IoError,
    },
    GeneratePdf {
        path: String,
        source: PdfGenerationError,
    },
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Wcif(e) => write!(f, "{e}"),
            AppError::Interactive(e) => write!(f, "{e}"),
            AppError::Planner(e) => write!(f, "{e}"),
            AppError::Split(e) => write!(f, "{e}"),
            AppError::CreatePdf { path, source } => {
                write!(f, "Error creating output PDF file {path}: {source}")
            }
            AppError::GeneratePdf { path, source } => {
                write!(f, "Error generating scorecards PDF {path}: {source}")
            }
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AppError::Wcif(e) => Some(e),
            AppError::Interactive(e) => Some(e),
            AppError::Planner(e) => Some(e),
            AppError::Split(e) => Some(e),
            AppError::CreatePdf { source, .. } => Some(source),
            AppError::GeneratePdf { source, .. } => Some(source),
        }
    }
}

impl From<WcifLoadError> for AppError {
    fn from(err: WcifLoadError) -> Self {
        AppError::Wcif(err)
    }
}

impl From<interactive::InteractiveError> for AppError {
    fn from(err: interactive::InteractiveError) -> Self {
        AppError::Interactive(err)
    }
}

impl From<PlannerError> for AppError {
    fn from(err: PlannerError) -> Self {
        AppError::Planner(err)
    }
}

impl From<SplitError> for AppError {
    fn from(err: SplitError) -> Self {
        AppError::Split(err)
    }
}

fn load_competition(source: &str) -> Result<wcif::Competition, wcif::WcifLoadError> {
    let spinner = progress::create_spinner(format!("Loading WCIF from: {source}..."));
    let comp_res = WcifLoader::load(source);
    spinner.finish_and_clear();
    let comp = comp_res?;
    println!("✔ Loaded competition: {} ({})", comp.name, comp.id);
    Ok(comp)
}

fn resolve_options(cli: &Cli, comp: &wcif::Competition) -> ResolvedOptions {
    let groupifier_config = comp.get_groupifier_config();
    ResolvedOptions::resolve(cli, groupifier_config.as_ref())
}

fn init_cli_and_competition() -> Result<(Cli, wcif::Competition), AppError> {
    if env::args().len() <= 1 {
        Ok(interactive::prompt_interactive_flow()?)
    } else {
        let parsed = Cli::parse();
        match parsed.comp_source.as_deref() {
            Some(source) => {
                let comp = load_competition(source)?;
                Ok((parsed, comp))
            }
            None => Ok(interactive::prompt_interactive_flow()?),
        }
    }
}

fn run() -> Result<(), AppError> {
    let (cli, comp) = init_cli_and_competition()?;

    let active_opts = resolve_options(&cli, &comp);
    println!("\n{active_opts}");

    let plan = ScorecardPlanner::plan(
        &comp,
        &cli.events,
        active_opts.cover_sheets,
        &active_opts.cover_sheets_by,
        active_opts.print_stations,
        active_opts.print_one_name,
        active_opts.local_names_first,
    )?;

    if plan.is_empty() {
        println!("No scorecards to generate.");
        return Ok(());
    }

    println!("{plan}");
    partition::generate_partitioned_pdfs(&comp, &plan.items, &active_opts)?;

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        process::exit(1);
    }
}
