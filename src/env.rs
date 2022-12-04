use chrono::Duration;
use crate::error::{ FestiveResult, FestiveError };

// environment variable handles
#[derive(Debug)]
pub enum Var { Leaderboard, Session, Notify, Status }

impl Var
{
    fn key(&self) -> &'static str
    {
        match self
        {
            Var::Leaderboard => "FESTIVE_BOT_LEADERBOARD",
            Var::Session     => "FESTIVE_BOT_SESSION",
            Var::Notify      => "FESTIVE_BOT_NOTIFY",
            Var::Status      => "FESTIVE_BOT_STATUS"
        }
    }

    pub fn get(self) -> FestiveResult<String>
    {
        std::env::var(self.key()).map_err(|_| FestiveError::Var(self))
    }
}

// command-line arguments
pub struct Args
{
    pub all_years: bool,
    pub period:    Duration,
    pub heartbeat: Option<Duration>
}

// states of the command-line argument parser
// also used to generate state-specific error messages
#[derive(Clone, Copy)]
enum ArgState { Period, Heartbeat }

impl ArgState
{
    // print state-specific error message and exit
    fn error(self) -> !
    {
        Args::usage();
        let err = match self
        {
            ArgState::Period    => "There was an error parsing the mins parameter for --period:\n\
                                    - The parameter is positive integer, representing the iteration period in minutes.\n\
                                    - The minimum accepted value is 15 minutes, and the maximum is 1440 minutes (one day).\n\
                                    - By default, if the argument is unset, the iteration period is one hour.",
            ArgState::Heartbeat => "There was an error parsing the mins parameter for --heartbeat:\n\
                                    - The parameter is positive integer, representing the interval between heartbeat messages in minutes.\n\
                                    - The maximum accepted value is 10080 minutes (one week), the minimum being limited by the iteration period (see --period).\n\
                                    - If not divisible by the iteration period (see `--period`), it will be rounded up to the next multiple.\n\
                                    - By default, if the argument is unset, no heartbeat messages will be sent."
        };
        println!("{err}");
        std::process::exit(1);
    }
}

impl Args
{
    fn usage()
    {
        println!("Usage: festive-bot [--all-years] [--period mins] [--heartbeat mins]");
    }

    // parse command-line arguments
    // exits the program with an error message
    pub fn parse() -> Args
    {
        let mut current = Args
        {
            all_years: false,
            period:    Duration::minutes(60),
            heartbeat: None
        };

        let mut state          = None;
        let mut period_mins    = current.period.num_minutes();
        let mut heartbeat_mins = None;

        for arg in std::env::args().skip(1)
        {
            match (arg.as_str(), state)
            {
                ("--all-years", None) => current.all_years = true,
                ("--period",    None) => state             = Some(ArgState::Period),
                ("--heartbeat", None) => state             = Some(ArgState::Heartbeat),

                // parse parameter for --period
                (param, Some(s@ArgState::Period)) =>
                {
                    period_mins     = param.parse::<i64>().ok().filter(|p| (15 ..= 1440).contains(p)).unwrap_or_else(|| s.error());
                    current.period  = Duration::minutes(period_mins);
                    state           = None;
                },

                // parse parameter for --heartbeat
                (param, Some(s@ArgState::Heartbeat)) =>
                {
                    heartbeat_mins = Some(param.parse::<i64>().ok().filter(|p| (1 ..= 1440 * 7).contains(p)).unwrap_or_else(|| s.error()));
                    state          = None;
                },

                // unexpected argument
                (arg, None) =>
                {
                    Args::usage();
                    println!("Encountered an unexpected command-line argument: {arg}");
                    std::process::exit(1);
                }
            }
        }

        // if state isn't None after parsing concludes, a parameter wasn't parsed
        if let Some(s) = state { s.error() }

        // calculate actual heartbeat duration now the period is known
        // if not divisible by period_mins, round up to the next multiple
        current.heartbeat = heartbeat_mins.map(|i| Duration::minutes((i + period_mins - 1) / period_mins * period_mins));
        current
    }
}
