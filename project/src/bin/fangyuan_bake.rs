use project::framework::fangyuan::{
    FangyuanBakeCliError, FangyuanBakeCliOptions, run_fangyuan_bake_cli,
};

fn main() {
    let options = match FangyuanBakeCliOptions::parse_from(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(FangyuanBakeCliError::Help(usage)) => {
            println!("{usage}");
            return;
        }
        Err(error) => {
            eprintln!("fangyuan_bake: {error}");
            std::process::exit(2);
        }
    };

    match run_fangyuan_bake_cli(&options) {
        Ok(report) => {
            println!(
                "fangyuan_bake: entries={} failed={} dry_run={}",
                report.entries.len(),
                report.failed_count(),
                report.dry_run
            );
            if !report.passed() {
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("fangyuan_bake: {error}");
            std::process::exit(1);
        }
    }
}
