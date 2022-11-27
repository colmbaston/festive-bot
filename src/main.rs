use json::JsonValue;
use reqwest::{ StatusCode, blocking::Client };
use chrono::{ Utc, DateTime, Datelike, TimeZone, Duration, DurationRound };
use num::{ FromPrimitive, ToPrimitive, rational::BigRational };
use std::{ fs::File, io::Read, path::PathBuf, collections::HashMap };

pub type FestiveResult<T> = Result<T, FestiveError>;

#[derive(Debug)]
pub enum FestiveError
{
    EnvVar(&'static str),
    Conversion,
    Filesystem,
    Http,
    Parse
}

fn main() -> FestiveResult<()>
{
    // environment variable names
    const LEADERBOARD : &str = "FESTIVE_BOT_LEADERBOARD";
    const SESSION     : &str = "FESTIVE_BOT_SESSION";
    const NOTIFY      : &str = "FESTIVE_BOT_NOTIFY";
    const STATUS      : &str = "FESTIVE_BOT_STATUS";

    // get mandatory environment variables
    let leaderboard = std::env::var(LEADERBOARD).map_err(|_| FestiveError::EnvVar(LEADERBOARD))?;
    let session     = std::env::var(SESSION).map_err(|_|     FestiveError::EnvVar(SESSION))?;

    // get optional environment variables
    let notify = std::env::var(NOTIFY).ok();
    let status = std::env::var(STATUS).ok();
    if notify.is_none() { println!("no NOTIFY webhook provided") }
    if status.is_none() { println!("no STATUS webhook provided") }

    // initiate the main loop
    let client    = Client::new();
    if let Err(e) = notify_cycle(&leaderboard, &session, notify.as_deref(), status.as_deref(), &client)
    {
        // attempt to send STATUS message for fatal error
        let _ = send_webhook(":warning: Festive Bot experienced an unrecoverable error and is exiting! :warning:", status.as_deref(), &client);
        let _ = send_webhook(&format!("Error: {e:?}"),                                                             status.as_deref(), &client);
        return Err(e)
    }
    Ok(())
}

fn notify_cycle(leaderboard : &str, session : &str, notify : Option<&str>, status : Option<&str>, client : &Client) -> FestiveResult<()>
{
    // reusable buffers for efficiency
    println!("initialising cycle");
    let mut events = Vec::new();
    let mut buffer = String::new();

    // populate currently-live AoC years
    let mut live = Vec::new();
    let mut prev = Utc::now();
    let year     = prev.year();
    live.extend(2015 .. year);
    if puzzle_unlock(year, 1)? <= prev { live.push(year) }

    // send AoC API requests only once every 15 minutes
    // use truncated timestamps to ensure complete coverage despite measurement imprecision
    let delay = Duration::minutes(15);
    prev      = prev.duration_trunc(delay).map_err(|_| FestiveError::Conversion)?;

    loop
    {
        // attempt to sleep until next iteration
        let current = prev + delay;
        println!("attempting to sleep until {current}");
        match (current - Utc::now()).to_std()
        {
            Ok(duration) => { std::thread::sleep(duration); println!("woke at {}", Utc::now()) },
            Err(_)       => println!("not sleeping, a previous iteration overran")
        }
        println!();

        // send heartbeat STATUS message every three hours
        let heartbeat = current.duration_trunc(Duration::hours(3)).map_err(|_| FestiveError::Conversion)?;
        if prev < heartbeat && heartbeat <= current
        {
            println!("sending {heartbeat} heartbeat");
            send_webhook(&format!(":information_source: Heartbeat {heartbeat}: Festive Bot is still alive! :hearts:"), status, client)?;
        }

        // extend live years if one has commenced this iteration
        let start = puzzle_unlock(current.year(), 1)?;
        if prev < start && start <= current { live.push(year) }

        for &year in &live
        {
            // send AoC API request, parsing the response to a vector of events
            println!("sending AoC API request for year {year}");
            let response = request_events(year, leaderboard, session, client)?;
            println!("parsing response");
            parse_events(&json::parse(&response).map_err(|_| FestiveError::Parse)?, &mut events)?;
            println!("parsed {} events", events.len());

            // read RFC 3339 timestamp from filesystem, defaulting to 28 days before current iteration
            let timestamp_path = PathBuf::from(format!("timestamp_{year}_{leaderboard}"));
            println!("reading {}", timestamp_path.display());
            let timestamp      = File::open(&timestamp_path).ok().and_then(|mut f|
            {
                buffer.clear();
                f.read_to_string(&mut buffer).ok()
                 .and_then(|_| DateTime::parse_from_rfc3339(buffer.trim()).ok())
                 .map(|dt| dt.with_timezone(&Utc))
            })
            .unwrap_or_else(|| { println!("timestamp read failed, defaulting to 28 days ago"); current - Duration::days(28) });
            println!("obtained timestamp {timestamp}");

            // NOTIFY for each puzzle event that took place after the latest timestamp, up to the start of this iteration
            for e in events.iter().skip_while(|e| e.timestamp <= timestamp).take_while(|e| e.timestamp < current)
            {
                send_webhook(&e.fmt()?, notify, client)?;
                println!("updating timestamp to {}", e.timestamp);

                std::fs::write(&timestamp_path, e.timestamp.to_rfc3339()).map_err(|_| FestiveError::Filesystem)?;
            }

            // make announcements once per day during December
            if current.year() == year && current.month() == 12
            {
                let day    = current.day();
                let puzzle = puzzle_unlock(year, day)?;
                if prev < puzzle && puzzle <= current
                {
                    // NOTIFY about a new AoC year
                    if day == 1
                    {
                        send_webhook(&format!(":christmas_tree: [{year}] Advent of Code is now live! :christmas_tree:"), notify, client)?
                    }

                    // NOTIFY about new puzzles
                    if day <= 25
                    {
                        send_webhook(&format!(":christmas_tree: [{year}] Puzzles for day {day:02} are now unlocked! :christmas_tree:"), notify, client)?;
                    }

                    // NOTIFY current leaderboard standings
                    let standings = if events.is_empty() { "No scores yet: get programming!".to_string() } else { standings(&events)? };
                    send_webhook(&format!(":christmas_tree: [{year}] Current Standings :christmas_tree:\n```{standings}```"), notify, client)?;
                }
            }
        }

        // roll over timestamps for next iteration
        prev = current;
        println!("completed iteration at {}", Utc::now());
    }
}

// puzzles unlock at 05:00 UTC each day from 1st to 25th December
// additionally used for daily standings announcements on the 26th to 31st December
fn puzzle_unlock(year : i32, day : u32) -> FestiveResult<DateTime<Utc>>
{
    Utc.with_ymd_and_hms(year, 12, day, 5, 0, 0).single().ok_or(FestiveError::Conversion)
}

// puzzle completion events parsed from AoC API
// year and day fields match corresponding components of DateTime<Utc>
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Event
{
    timestamp: DateTime<Utc>,
    year:      i32,
    day:       u32,
    star:      u8,
    id:        Identifier
}

// unique identifier for participant on this leaderboard
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Identifier
{
    name:    String,
    numeric: u64
}

impl Event
{
    // not using Display trait so FestiveResult can be returned
    fn fmt(&self) -> FestiveResult<String>
    {
        let (parts, stars) = match self.star
        {
            1 => ("the first part", ":star:"),
            2 => ("both parts",     ":star: :star:"),
            _ => return Err(FestiveError::Parse)
        };

        let score  = self.score()?;
        let plural = if score == num::one() { "" } else { "s" };
        Ok(format!(":christmas_tree: [{}] {} has completed {parts} of puzzle {:02}, scoring {score} point{plural}! {stars}", self.year, self.id.name, self.day))
    }

    // custom scoring based on the reciprocal of full days since the puzzle was released
    fn score(&self) -> FestiveResult<BigRational>
    {
        let days                = (self.timestamp - puzzle_unlock(self.year, self.day)?).num_days();
        let ratio : BigRational = FromPrimitive::from_i64(1 + days).ok_or(FestiveError::Conversion)?;
        Ok(ratio.recip())
    }
}

fn request_events(year : i32, leaderboard : &str, session : &str, client : &Client) -> FestiveResult<String>
{
    let url = format!("https://adventofcode.com/{year}/leaderboard/private/view/{leaderboard}.json");

    // send HTTP request
    let response = client.get(&url)
                         .header("cookie", format!("session={session}"))
                         .send()
                         .map_err(|_| FestiveError::Http)?;

    match response.status()
    {
        StatusCode::OK => response.text().map_err(|_| FestiveError::Http),

        // unexpected status code
        // INTERNAL_SERVER_ERROR probably means the session cookie expired
        _ => Err(FestiveError::Http)
    }
}

fn parse_events(json : &JsonValue, events : &mut Vec<Event>) -> FestiveResult<()>
{
    events.clear();

    for (id, member) in json["members"].entries()
    {
        let name = member["name"].to_string();

        for (day, stars) in member["completion_day_level"].entries()
        {
            for (star, contents) in stars.entries()
            {
                events.push(Event
                {
                    timestamp: Utc.timestamp_opt(contents["get_star_ts"].as_i64().ok_or(FestiveError::Parse)?, 0).single().ok_or(FestiveError::Conversion)?,
                    year:      json["event"].to_string().parse().map_err(|_| FestiveError::Parse)?,
                    day:       day.parse().map_err(|_| FestiveError::Parse)?,
                    star:      star.parse().map_err(|_| FestiveError::Parse)?,
                    id:        Identifier { name: name.clone(), numeric: id.parse().map_err(|_| FestiveError::Parse)? }
                });
            }
        }
    }

    events.sort_unstable();
    Ok(())
}

fn score_events(events : &[Event]) -> FestiveResult<Vec<(&Identifier, BigRational)>>
{
    // score histogram
    let mut hist : HashMap<&Identifier, BigRational> = HashMap::new();
    for e in events
    {
        *hist.entry(&e.id).or_insert_with(num::zero) += e.score()?;
    }

    // sort by score descending, then by Identifier ascending
    let mut scores = hist.into_iter().collect::<Vec<_>>();
    scores.sort_unstable_by_key(|(id, score)| (-score, *id));
    Ok(scores)
}

fn standings(events : &[Event]) -> FestiveResult<String>
{
    // calculate maximum-length name for space padding
    let scores   = score_events(events)?;
    let max_name = scores.iter().map(|(id, _)| id.name.len()).max().unwrap_or(0);

    // generate standings report, with one line per participant
    let mut standings = String::new();
    for (id, score) in scores
    {
        standings.push_str(&format!("{}:", id.name));
        for _ in id.name.len() ..= max_name { standings.push(' ') }
        standings.push_str(&format!("{:>5.02}", score.to_f64().ok_or(FestiveError::Conversion)?));
        standings.push('\n');
    }
    Ok(standings)
}

fn send_webhook(payload : &str, webhook : Option<&str>, client : &Client) -> FestiveResult<()>
{
    println!("webhook: {:?}", payload);

    // only send HTTP request if webhook actually contains a URL
    if let Some(url) = webhook
    {
        let json = json::object!{ content: payload };

        loop
        {
            // send request
            let response = client.post(url)
                                 .header("Content-Type", "application/json")
                                 .body(json.to_string())
                                 .send()
                                 .map_err(|_| FestiveError::Http)?;

            match response.status()
            {
                // expected status codes for successful request
                StatusCode::OK | StatusCode::NO_CONTENT => break,

                // keep retrying request until rate-limiting period ends
                StatusCode::TOO_MANY_REQUESTS =>
                {
                    let retry = json::parse(&response.text().map_err(|_| FestiveError::Http)?).map_err(|_| FestiveError::Parse)?["retry_after"].as_f32().unwrap_or(0.0);
                    println!("rate-limited for {}s", retry);
                    std::thread::sleep(std::time::Duration::from_millis((retry * 1000.0) as u64));
                },

                // unexpected status code
                _ => return Err(FestiveError::Http)
            }

            println!("retrying");
        }
    }
    Ok(())
}
