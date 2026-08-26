use project::framework::fangyuan::{
    FangyuanAuthoringCliError, FangyuanAuthoringCliOptions, fangyuan_authoring_cli_error_exit_code,
    format_fangyuan_authoring_cli_report, run_fangyuan_authoring_cli_in_repository,
};

fn main() {
    let options = match FangyuanAuthoringCliOptions::parse_from(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(FangyuanAuthoringCliError::Help(usage)) => {
            println!("{usage}");
            return;
        }
        Err(error) => {
            eprintln!("fangyuan_authoring: {error}");
            std::process::exit(fangyuan_authoring_cli_error_exit_code(&error));
        }
    };

    match run_fangyuan_authoring_cli_in_repository(&options) {
        Ok(report) => println!("{}", format_fangyuan_authoring_cli_report(&report)),
        Err(error) => {
            eprintln!("fangyuan_authoring: {error}");
            std::process::exit(fangyuan_authoring_cli_error_exit_code(&error));
        }
    }
}
