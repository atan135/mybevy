use std::{env, io, process::ExitCode};

use project::lockstep_sim_headless::{
    HeadlessCommand, HeadlessOptions, failure_run, parse_headless_args, run_headless, write_jsonl,
};

fn main() -> ExitCode {
    let command = match parse_headless_args(env::args().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            let run = failure_run(
                &HeadlessOptions::default(),
                error.error_code,
                error.failure_stage,
                error.message,
                error.exit_code,
            );
            return write_run(&run);
        }
    };

    match command {
        HeadlessCommand::Help => {
            print!("{}", project::lockstep_sim_headless::HEADLESS_HELP);
            ExitCode::SUCCESS
        }
        HeadlessCommand::Run(options) => write_run(&run_headless(&options)),
    }
}

fn write_run(run: &project::lockstep_sim_headless::HeadlessRun) -> ExitCode {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    if let Err(error) = write_jsonl(&mut output, &run.records) {
        eprintln!("lockstep headless telemetry write failed: {error}");
        return ExitCode::from(70);
    }

    ExitCode::from(run.exit_code)
}
