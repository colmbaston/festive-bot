#![feature(slice_group_by)]

use std::{ fs::File, io::Read, path::PathBuf };
use chrono::{ Utc, DateTime, Datelike, Duration };
use reqwest::blocking::Client;

mod error;
use error::{ FestiveError, FestiveResult };

mod env;
use env::{ Var, Args };

mod event;
use event::Event;

mod webhook;
use webhook::Webhook;

fn main()
{
    if let Err(e) = initialise()
    {
        println!("{e}");
        std::process::exit(1)
    }
}

fn initialise() -> FestiveResult<()>
{
    // mandatory environment variables
    let leaderboard = Var::Leaderboard.get()?;
    let session     = Var::Session.get()?;

    // parse command-line arguments
    let args = Args::parse();

    // HTTP client with appropriate user agent
    let client = Client::builder().user_agent(format!("Festive Bot v{}; https://crates.io/festive-bot; colm@colmbaston.uk", env!("CARGO_PKG_VERSION")))
                                  .build().map_err(|_| FestiveError::Init)?;

    // initiate the main loop
    let result = notify_cycle(&leaderboard, &session, &args, &client);
    if let Err(e) = &result
    {
        // attempt to send status message about fatal error
        // ignore these results, as the program is already exiting
        let _ = Webhook::send("⚠ Festive Bot experienced an unrecoverable error, exiting!", &[], Webhook::Status, &client);
        let _ = Webhook::send(&format!("⚠ Error: {e:?}"),                                   &[], Webhook::Status, &client);
    }
    result
}

fn notify_cycle(leaderboard : &str, session : &str, args : &Args, client : &Client) -> FestiveResult<()>
{
    // status message notifying about initilisation
    println!("initialising");
    Webhook::send(&format!("🦀 Festive Bot v{} is initialising...", env!("CARGO_PKG_VERSION")), &[], Webhook::Status, client)?;

    // set handler for POSIX termination signals
    // hander needs to own the HTTP client it uses, so give it a clone
    println!("setting handler for SIGINT, SIGTERM, and SIGHUP signals");
    let handler_client = client.clone();
    ctrlc::set_handler(move ||
    {
        println!("received termination signal, exiting...");
        let _ = Webhook::send("🦀 Received termination signal, exiting!", &[], Webhook::Status, &handler_client);
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

    // use truncated timestamps to ensure complete coverage despite measurement imprecision
    prev = Event::trunc_ts(&prev, args.period)?;

    // reusable buffers for efficiency
    let mut events = Vec::new();
    let mut buffer = String::new();

    println!("initialisation successful");
    Webhook::send("🦀 Initialisation successful!",
                  &[("params.txt", format!("leaderboard: {leaderboard}\n\
                                            all years:   {}\n\
                                            period:      {}\n\
                                            standings:   {}\n\
                                            heartbeat    {:?}\n\
                                            live years:  {live:?}\n",
                                            args.all_years,
                                            args.period.num_minutes(),
                                            args.standings.num_minutes(),
                                            args.heartbeat.map(|d| d.num_minutes())).as_bytes())],
                  Webhook::Status, client)?;

    loop
    {
        // attempt to sleep until next iteration
        let current = prev + args.period;
        year        = current.year();
        println!("attempting to sleep until {current}");
        match (current - Utc::now()).to_std()
        {
            Ok(duration) => { std::thread::sleep(duration); println!("woke at {}", Utc::now()) },
            Err(_)       => println!("not sleeping, a previous iteration overran")
        }
        println!();

        // if a timestamp has occurred since the previous iteration, it can trigger something to happen this iteration
        let trigger = |ts| prev < ts && ts <= current;

        // send heartbeat status message when heartbeat is set
        if let Some(heartbeat_dur) = args.heartbeat
        {
            let heartbeat_ts = Event::trunc_ts(&current, heartbeat_dur)?;
            if trigger(heartbeat_ts)
            {
                Webhook::send(&format!("🦀 Heartbeat {heartbeat_ts}"), &[], Webhook::Status, client)?;
            }
        }

        // extend live years if puzzle one of this year has unlocked
        if trigger(Event::puzzle_unlock(year, 1)?) && live.binary_search(&year).is_err()
        {
            live.push(year);
            Webhook::send(&format!("🦀 Adding {year} to live years!"), &[], Webhook::Status, client)?;
        }

        // only report on past years when all_years is set
        for &request_year in live.iter().filter(|&y| args.all_years || y == &year)
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

            // announcements made only during December
            if request_year == year && current.month() == 12
            {
                // daily puzzle-unlock announcement
                let day = current.day();
                if day <= 25 && trigger(Event::puzzle_unlock(year, day)?)
                {
                    // new AoC year announcement
                    if day == 1
                    {
                        Webhook::send(&format!("🎄 [{year}] Advent of Code is now live! 🎉"), &[], Webhook::Notify, client)?
                    }

                    // new puzzle announcement
                    Webhook::send(&format!("🎄 [{year}] Puzzle {day:02} is now unlocked! 🔓"), &[], Webhook::Notify, client)?;
                }

                // leaderboard standings announcement
                if trigger(Event::trunc_ts(&current, args.standings)?)
                {
                    let standings = if events.is_empty() { "No scores yet: get programming!\n".to_string() } else { Event::standings(&events)? };
                    Webhook::send(&format!("🎄 [{year}] Current Standings 🏆"), &[(&format!("standings_{year}_12_{day:02}.txt"), standings.as_bytes())], Webhook::Notify, client)?;
                }

                // sign off for the year
                if (current + args.period).year() != request_year
                {
                    Webhook::send(&format!("🎄 [{year}] Festive Bot signing off. Happy New Year! 👋"), &[], Webhook::Notify, client)?;
                }
            }
        }

        // roll over timestamps for next iteration
        prev = current;
        println!("completed iteration at {}", Utc::now());
    }
}
