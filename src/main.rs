mod options;
mod pdf;
mod progress;
mod scorecard;
mod wcif;

use clap::Parser;
use mimalloc::MiMalloc;
use options::{Cli, ResolvedOptions};
use pdf::{PageLayout, PdfGenerator};
use scorecard::ScorecardPlanner;
use std::fs::File;
use std::io::BufWriter;
use std::process;
use wcif::WcifLoader;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

const BUFFER_SIZE: usize = 16 * 1024 * 1024; // 16 MB

fn main() {
    let cli = Cli::parse();

    if let Err(err) = cli.validate() {
        eprintln!("Error: {}", err);
        process::exit(1);
    }

    println!("Loading WCIF from: {}...", cli.comp_source);
    let comp = match WcifLoader::load(&cli.comp_source) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Error loading WCIF: {}", err);
            process::exit(1);
        }
    };

    println!("Loaded competition: {} ({})", comp.name, comp.id);

    // Resolve active options by merging CLI options, WCIF extensions, and defaults
    let groupifier_config = comp.get_groupifier_config();
    let active_opts = ResolvedOptions::resolve(&cli, groupifier_config.as_ref());
    active_opts.print_summary();

    // Plan scorecards according to requested events or open rounds
    let scorecards = match ScorecardPlanner::plan(&comp, &cli.events) {
        Ok(cards) => cards,
        Err(err) => {
            eprintln!("Error planning scorecards: {}", err);
            process::exit(1);
        }
    };

    if scorecards.is_empty() {
        println!("No scorecards to generate.");
        return;
    }

    let layout = PageLayout::from_paper_size(&active_opts.paper);
    let generator = PdfGenerator::new(layout);

    let out_filename = format!("{}-scorecards.pdf", comp.id);
    let file = match File::create(&out_filename) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("Error creating output PDF file {}: {}", out_filename, err);
            process::exit(1);
        }
    };
    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);

    let spinner = progress::create_spinner(format!(
        "Generating {} scorecards to PDF ({})...",
        scorecards.len(),
        out_filename
    ));

    if let Err(err) = generator.generate_to_writer(&comp, &scorecards, &mut writer) {
        spinner.finish_and_clear();
        eprintln!("Error generating scorecards PDF: {}", err);
        process::exit(1);
    }
    spinner.finish_and_clear();

    println!("Successfully generated scorecards PDF: {}", out_filename);
}
