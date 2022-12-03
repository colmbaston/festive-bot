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
            ArgState::Period    => "There was an error parsing a parameter for --period:\n\
                                    - Parameter p is positive integer, representing a number of 15-minute intervals.\n\
                                    - The default iteration period is the minimum 15 minutes.\n\
                                    - For example, if p is 4, an iteration will be run once per hour.",
            ArgState::Heartbeat => "There was an error parsing a parameter for --heartbeat:\n\
                                    - Parameter i is positive integer, representing a number of iteration periods.\n\
                                    - By default, no hearbeat status messages are sent; if i is set, they are sent every i iterations.\n\
                                    - For example, if the period is 30 minutes and i is 6, a heartbeat will be sent every three hours."
        };
        println!("{err}");
        std::process::exit(1);
    }
}

impl Args
{
    fn usage()
    {
        println!("Usage: festive-bot [--all-years] [--period p] [--heartbeat i]");
    }

    // parse command-line arguments
    // exits the program with an error message
    pub fn parse() -> Args
    {
        let mut current = Args
        {
            all_years: false,
            period:    Duration::minutes(15),
            heartbeat: None
        };

        let mut state       = None;
        let mut heartbeat_i = None;

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
                    let p          = param.parse::<i32>().ok().filter(|&p| 1 <= p).unwrap_or_else(|| s.error());
                    current.period = Duration::minutes(15 * p as i64);
                    state          = None;
                },

                // parse parameter for --heartbeat
                (param, Some(s@ArgState::Heartbeat)) =>
                {
                    heartbeat_i = Some(param.parse::<i32>().ok().filter(|&p| 1 <= p).unwrap_or_else(|| s.error()));
                    state       = None;
                },

                // unexpected argument
                (arg, None) =>
                {
                    Args::usage();
                    println!("There was an unexpected argument: {arg}");
                    std::process::exit(1);
                }
            }
        }

        // if state isn't None after parsing concludes, a parameter wasn't parsed
        if let Some(s) = state { s.error() }

        // calculate actual heartbeat duration now the period is known
        current.heartbeat = heartbeat_i.map(|i| current.period * i);
        current
    }
}
