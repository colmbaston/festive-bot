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

// options passed as command-line arguments
// also used as states for the argument parser
#[derive(Clone, Copy)]
enum Opt { AllYears, Period, Heartbeat }

impl Opt
{
    // describe this option's paramters
    fn usage(self) -> &'static str
    {
        match self
        {
            Opt::AllYears  => "[--all-years]",
            Opt::Period    => "[--period mins]",
            Opt::Heartbeat => "[--heartbeat mins]"
        }
    }

    // print option-specific error message and exit the process
    fn error(self) -> !
    {
        Args::usage();
        let err = match self
        {
            // no error message as --all-years has no parameters
            Opt::AllYears  => "",

            // the mins parameter of --period
            Opt::Period    => "There was an error parsing the mins parameter of --period:\n\
                               - The mins parameter should be a positive integer, representing the iteration period in minutes.\n\
                               - The minimum accepted value is 15 minutes, and the maximum is 1440 minutes (one day).\n\
                               - If mins doesn't divide evenly into one day, it will be rounded up to the next factor.\n\
                               - By default, if --period is unset, the iteration period is one hour.\n",

            // the mins parameter of --heartbeat
            Opt::Heartbeat => "There was an error parsing the mins parameter of --heartbeat:\n\
                               - The mins parameter should be a positive integer, representing the interval between heartbeat messages in minutes.\n\
                               - The maximum accepted value is 10080 minutes (one week), the minimum being limited by the iteration period (see --period).\n\
                               - If mins isn't divisible by the iteration period, it will be rounded up to the next multiple.\n\
                               - By default, if --heartbeat is unset, no heartbeat messages will be sent.\n"
        };
        print!("{err}");
        std::process::exit(1);
    }

    // iterate through all options
    fn iter() -> impl Iterator<Item = Opt>
    {
        [Opt::AllYears, Opt::Period, Opt::Heartbeat].into_iter()
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

    // parse command-line arguments
    // exists the process with an error message if parsing fails
    pub fn parse() -> Args
    {
        let mut current = Args
        {
            all_years: false,
            period:    Duration::minutes(60),
            heartbeat: None
        };

        let mut state          = None;
        let mut mins_period    = current.period.num_minutes() as i16;
        let mut mins_heartbeat = None;
        for arg in std::env::args().skip(1)
        {
            match (arg.as_str(), state)
            {
                ("--all-years", None) => current.all_years = true,
                ("--period",    None) => state             = Some(Opt::Period),
                ("--heartbeat", None) => state             = Some(Opt::Heartbeat),

                // parse mins parameter for --period
                (mins, Some(s@Opt::Period)) =>
                {
                    mins_period = mins.parse::<i16>().ok().filter(|p| (15 ..= 1440).contains(p)).unwrap_or_else(|| s.error());
                    state       = None;
                },

                // parse mins parameter for --heartbeat
                (mins, Some(s@Opt::Heartbeat)) =>
                {
                    mins_heartbeat = Some(mins.parse::<i16>().ok().filter(|p| (1 ..= 1440 * 7).contains(p)).unwrap_or_else(|| s.error()));
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

        // round mins_period up to next factor of 1440 minutes (one day)
        // round mins_heartbeat up to next value which is a multiple of mins_period
        let factors       = [15,16,18,20,24,30,32,36,40,45,48,60,72,80,90,96,120,144,160,180,240,288,360,480,720,1440];
        current.period    = Duration::minutes(factors[factors.binary_search(&mins_period).map_or_else(|i| i, |i| i)]     as i64);
        current.heartbeat = mins_heartbeat.map(|i| Duration::minutes(((i + mins_period - 1) / mins_period * mins_period) as i64));
        current
    }
}
