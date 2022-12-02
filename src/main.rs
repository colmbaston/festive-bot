#![feature(slice_group_by)]

use std::{ fs::File, io::Read, path::PathBuf };
use chrono::{ Utc, DateTime, Datelike, Duration, DurationRound };
use reqwest::blocking::Client;

mod error;
use error::{ FestiveError, FestiveResult, EnvVar };

mod event;
use event::Event;

mod webhook;
use webhook::Webhook;

fn main() -> FestiveResult<()>
{
    // mandatory environment variables
    let leaderboard = EnvVar::Leaderboard.get()?;
    let session     = EnvVar::Session.get()?;

    // parse command-line args
    let mut all_years = false;
    let mut heartbeat = false;
    let mut args      = std::env::args();
    let prog          = args.next().unwrap_or_else(|| "festive-bot".to_string());
    for arg in args
    {
        match arg.as_str()
        {
            "--all-years" => all_years = true,
            "--heartbeat" => heartbeat = true,
            _             => { println!("usage: {prog} [--all-years] [--heartbeat]"); return Err(FestiveError::Arg) }
        }
    }

    // HTTP client with appropriate user agent
    let client = Client::builder().user_agent(format!("Festive Bot v{}; https://crates.io/festive-bot; colm@colmbaston.uk", env!("CARGO_PKG_VERSION")))
                                  .build().map_err(|_| FestiveError::Init)?;

    // initiate the main loop
    if let Err(e) = notify_cycle(&leaderboard, &session, &client, all_years, heartbeat)
    {
        // attempt to send status message about fatal error
        // ignore these results, as the program is already exiting
        let _ = Webhook::send(":warning: Festive Bot experienced an unrecoverable error, exiting!", &[], Webhook::Status, &client);
        let _ = Webhook::send(&format!(":warning: Error: {e:?}"),                                   &[], Webhook::Status, &client);
        return Err(e)
    }
    Ok(())
}

fn notify_cycle(leaderboard : &str, session : &str, client : &Client, all_years : bool, heartbeat : bool) -> FestiveResult<()>
{
    // status message notifying about initilisation
    println!("initialising");
    Webhook::send(&format!(":crab: Festive Bot v{} is initialising...", env!("CARGO_PKG_VERSION")), &[], Webhook::Status, client)?;

    // set handler for POSIX termination signals
    // hander needs to own the HTTP client it uses, so give it a clone
    println!("setting handler for SIGINT, SIGTERM, and SIGHUP signals");
    let handler_client = client.clone();
    ctrlc::set_handler(move ||
    {
        println!("received termination signal, exiting...");
        let _ = Webhook::send(":crab: Received termination signal, exiting! :wave:", &[], Webhook::Status, &handler_client);
        std::process::exit(0);
    })
    .map_err(|_| FestiveError::Init)?;

    // populate currently-live AoC years
    println!("determining currently-live AoC years");
    let mut live = Vec::new();
    let mut prev = Utc::now();
    let mut year = prev.year();
    live.extend(2015 .. year);
    if Event::puzzle_unlock(year, 1).map_err(|_| FestiveError::Init)? <= prev { live.push(year) }

    // send AoC API requests only once every 15 minutes
    // use truncated timestamps to ensure complete coverage despite measurement imprecision
    let period = Duration::minutes(15);
    prev       = prev.duration_trunc(period).map_err(|_| FestiveError::Init)?;

    // reusable buffers for efficiency
    let mut events = Vec::new();
    let mut buffer = String::new();

    println!("initialisation successful");
    Webhook::send(&format!(":crab: Initialisation successful! Monitoring leaderboard {leaderboard}... :eyes:"), &[], Webhook::Status, client)?;

    loop
    {
        // attempt to sleep until next iteration
        let current = prev + period;
        year        = current.year();
        println!("attempting to sleep until {current}");
        match (current - Utc::now()).to_std()
        {
            Ok(duration) => { std::thread::sleep(duration); println!("woke at {}", Utc::now()) },
            Err(_)       => println!("not sleeping, a previous iteration overran")
        }
        println!();

        // if a timestamp has occurred since the previous iteration, it can trigger something to happen this iteration
        let trigger = |timestamp| prev < timestamp && timestamp <= current;

        // send status message every hour when heartbeat is set
        if heartbeat
        {
            let heartbeat_timestamp = current.duration_trunc(Duration::hours(1)).map_err(|_| FestiveError::Conv)?;
            if trigger(heartbeat_timestamp)
            {
                Webhook::send(&format!(":crab: Heartbeat {heartbeat_timestamp} :heart:"), &[], Webhook::Status, client)?;
            }
        }

        // extend live years if puzzle one of this year has unlocked
        if trigger(Event::puzzle_unlock(year, 1)?) && live.binary_search(&year).is_err()
        {
            live.push(year);
            Webhook::send(&format!(":crab: Adding {year} to live years!"), &[], Webhook::Status, client)?;
        }

        // only report on past years when all_years is set
        for &request_year in live.iter().filter(|&y| all_years || y == &year)
        {
            // send AoC API request, parsing the response to a vector of events
            println!("sending AoC API request for year {request_year}");
            let response = Event::request(request_year, leaderboard, session, client)?;
            println!("parsing response");
            Event::parse(&response, &mut events)?;
            println!("parsed {} events", events.len());

            // read RFC 3339 timestamp from filesystem, defaulting to 28 days before current iteration
            let timestamp_path = PathBuf::from(format!("timestamp_{request_year}_{leaderboard}"));
            println!("reading {}", timestamp_path.display());
            let timestamp = File::open(&timestamp_path).ok().and_then(|mut f|
            {
                buffer.clear();
                f.read_to_string(&mut buffer).ok()
                 .and_then(|_| DateTime::parse_from_rfc3339(buffer.trim()).ok())
                 .map(|dt| dt.with_timezone(&Utc))
            })
            .unwrap_or_else(||
            {
                println!("timestamp read failed, defaulting to 28 days ago");
                current - Duration::days(28)
            });
            println!("obtained timestamp {timestamp}");

            // message for each puzzle event that took place after the latest timestamp, up to the start of this iteration
            for e in events.iter().skip_while(|e| e.timestamp() <= &timestamp).take_while(|e| e.timestamp() < &current)
            {
                Webhook::send(&e.fmt()?, &[], Webhook::Notify, client)?;
                println!("updating timestamp to {}", e.timestamp());
                std::fs::write(&timestamp_path, e.timestamp().to_rfc3339()).map_err(|_| FestiveError::File)?;
            }

            // make announcements once per day during December
            if request_year == year && current.month() == 12
            {
                let day = current.day();
                if trigger(Event::puzzle_unlock(year, day)?)
                {
                    // message about a new AoC year
                    if day == 1
                    {
                        Webhook::send(&format!(":christmas_tree: [{year}] Advent of Code is now live! :tada:"), &[], Webhook::Notify, client)?
                    }

                    // message about new puzzle
                    if day <= 25
                    {
                        Webhook::send(&format!(":christmas_tree: [{year}] Puzzle {day:02} is now unlocked! :unlock:"), &[], Webhook::Notify, client)?;
                    }

                    // message with current leaderboard standings
                    let standings = if events.is_empty() { "No scores yet: get programming!\n".to_string() } else { Event::standings(&events)? };
                    Webhook::send(&format!(":christmas_tree: [{year}] Current Standings :trophy:"), &[(&format!("standings_{year}_12_{day:02}.txt"), standings.as_bytes())], Webhook::Notify, client)?;
                }
            }
        }

        // roll over timestamps for next iteration
        prev = current;
        println!("completed iteration at {}", Utc::now());
    }
}
