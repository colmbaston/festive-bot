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
    pub standings: Duration,
    pub heartbeat: Option<Duration>
}

// useful durations in minutes
const HOUR : i64 = 60;
const DAY  : i64 = HOUR * 24;
const WEEK : i64 = DAY  * 7;

// options passed as command-line arguments
// also used as states for the argument parser
#[derive(Clone, Copy)]
enum Opt { AllYears, Period, Standings, Heartbeat }

impl Opt
{
    // describe this option's paramters
    fn usage(self) -> &'static str
    {
        match self
        {
            Opt::AllYears  => "[--all-years]",
            Opt::Period    => "[--period mins]",
            Opt::Standings => "[--standings mins]",
            Opt::Heartbeat => "[--heartbeat mins]"
        }
    }

    // print option-specific error message and exit the process
    fn error(self) -> !
    {
        Args::usage();
        println!("There was an error with {}:", self.usage());

        match self
        {
            // no error message as there are no parameters for --all-years
            Opt::AllYears => (),

            // the mins parameter of --period
            Opt::Period =>
            {
                println!("- The mins parameter should be a positive integer, representing the iteration period in minutes.");
                println!("- The minimum value is 15 minutes, and it must divide evenly into {DAY} (one day).");
                println!("- If unset, the default value is {HOUR} (one hour).");
            },

            // the mins parameter of --standings
            Opt::Standings =>
            {
                println!("- The mins parameter should be a positive integer, representing the interval between standings announcements in minutes.");
                println!("- It must be a multiple of the iteration period (see --period), and be no larger than {WEEK} (one week).");
                println!("- If unset, the default value is {DAY} (one day).");
            },

            // the mins parameter of --heartbeat
            Opt::Heartbeat =>
            {
                println!("- The mins parameter should be a positive integer, representing the interval between heartbeat messages in minutes.");
                println!("- It must be a multiple of the iteration period (see --period), and be no larger than {WEEK} (one week).");
                println!("- If unset, no heartbeat messages are sent.");
            }
        };
        std::process::exit(1);
    }

    // iterate through all options
    fn iter() -> impl Iterator<Item = Opt>
    {
        [Opt::AllYears,
         Opt::Period,
         Opt::Standings,
         Opt::Heartbeat].into_iter()
    }
}

impl Args
{
    // print usage for all options
    fn usage()
    {
        print!("Usage: festive-bot");
        for opt in Opt::iter() { print!(" {}", opt.usage()) }
        println!();
    }

    fn new() -> Args
    {
        Args
        {
            all_years: false,
            period:    Duration::minutes(HOUR),
            standings: Duration::minutes(DAY),
            heartbeat: None
        }
    }

    // parse command-line arguments
    // exists the process with an error message if parsing fails
    pub fn parse() -> Args
    {
        let mut current        = Args::new();
        let mut state          = None;
        let mut mins_period    = current.period.num_minutes();
        let mut mins_standings = current.standings.num_minutes();
        let mut mins_heartbeat = None;
        for arg in std::env::args().skip(1)
        {
            match (arg.as_str(), state)
            {
                ("--all-years", None) => current.all_years = true,
                ("--period",    None) => state             = Some(Opt::Period),
                ("--standings", None) => state             = Some(Opt::Standings),
                ("--heartbeat", None) => state             = Some(Opt::Heartbeat),

                // parse mins parameter for --period
                (mins, Some(s@Opt::Period)) =>
                {
                    mins_period = mins.parse::<i64>().ok().filter(|&m| 15 <= m && DAY % m == 0).unwrap_or_else(|| s.error());
                    state       = None;
                },

                // parse mins parameter for --standings
                (mins, Some(s@Opt::Standings)) =>
                {
                    mins_standings = mins.parse::<i64>().ok().filter(|&m| m <= WEEK).unwrap_or_else(|| s.error());
                    state          = None;
                },

                // parse mins parameter for --heartbeat
                (mins, Some(s@Opt::Heartbeat)) =>
                {
                    mins_heartbeat = Some(mins.parse::<i64>().ok().filter(|&m| m <= WEEK).unwrap_or_else(|| s.error()));
                    state          = None;
                },

                // unexpected argument
                (arg, _) =>
                {
                    Args::usage();
                    println!("Encountered an unexpected command-line argument: {arg}");
                    std::process::exit(1);
                }
            }
        }

        // if state isn't None after parsing concludes, a parameter wasn't parsed
        if let Some(s) = state { s.error() }

        // now the actual iteration period is known, ensure --standings and --heartbeat parameters are multiples of it
        if                                      mins_standings % mins_period != 0 { Opt::Standings.error() }
        if let Some(mins) = mins_heartbeat { if mins           % mins_period != 0 { Opt::Heartbeat.error() }}

        current.period    = Duration::minutes(mins_period);
        current.standings = Duration::minutes(mins_standings);
        current.heartbeat = mins_heartbeat.map(Duration::minutes);
        current
    }
}
