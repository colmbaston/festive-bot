use json::JsonValue;
use reqwest::{ StatusCode, blocking::Client };
use chrono::{ Utc, DateTime, Datelike, TimeZone, Duration, DurationRound };
use num::{ FromPrimitive, ToPrimitive, rational::BigRational };
use std::{ fs::File, io::Read, error::Error, fmt::{ Display, Formatter }, collections::HashMap };

fn main() -> Result<(), Box<dyn Error>>
{
    let leaderboard = std::env::var("FESTIVE_BOT_LEADERBOARD")?;
    let session     = std::env::var("FESTIVE_BOT_SESSION")?;
    let webhook     = std::env::var("FESTIVE_BOT_WEBHOOK")?;

    let client    = Client::new();
    if let Err(e) = update_loop(&leaderboard, &session, &webhook, &client)
    {
        let _ = send_webhook(&webhook, &client, ":christmas_tree: Festive Bot encountered an error and is exiting! :warning:");
        return Err(e)
    }
    Ok(())
}

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

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Identifier
{
    name:    String,
    numeric: u64
}

impl Display for Event
{
    fn fmt(&self, f : &mut Formatter) -> std::fmt::Result
    {
        let (parts, stars) = match self.star
        {
            1 => ("the first part", ":star:"),
            2 => ("both parts",     ":star: :star:"),
            _ => panic!("cannot display puzzle event with star {}", self.star)
        };

        let score  = self.score();
        let plural = if score == FromPrimitive::from_u8(1).unwrap() { "" } else { "s" };
        write!(f, ":christmas_tree: [{}] {} has completed {parts} of puzzle {:02}, scoring {score} point{plural}! {stars}", self.year, self.id.name, self.day)
    }
}

impl Event
{
    fn days_to_complete(&self) -> i64
    {
        (self.timestamp - puzzle_unlock(self.year, self.day)).num_days()
    }

    // custom scoring based on the reciprocal of full days since the puzzle was released
    fn score(&self) -> BigRational
    {
        let ratio : BigRational = FromPrimitive::from_i64(1 + self.days_to_complete()).unwrap();
        ratio.recip()
    }
}

fn update_loop(leaderboard : &str, session : &str, webhook : &str, client : &Client) -> Result<(), Box<dyn Error>>
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
    if puzzle_unlock(year, 1) <= prev { live.push(year) }

    // send API requests only once every 15 minutes
    // use truncated timestamps to ensure complete coverage despite measurement imprecision
    let delay = Duration::minutes(15);
    prev      = prev.duration_trunc(delay)?;

    loop
    {
        // attempt to sleep until next iteration
        let next = prev + delay;
        match (next - Utc::now()).to_std()
        {
            Ok(duration) =>
            {
                println!("sleeping until {next}");
                std::thread::sleep(duration);
                println!("woke at {}", Utc::now());
            },
            Err(_) => println!("an iteration overran the delay duration, catching up")
        }
        println!();

        // extend live years if one has commenced this iteration
        let start = puzzle_unlock(next.year(), 1);
        if prev < start && start <= next { live.push(year) }

        for &year in &live
        {
            // send API request to AoC, parse and vectorise the results
            println!("sending API request for year {year}");
            let response = request_events(year, leaderboard, session, client)?;
            println!("parsing response");
            vectorise_events(&json::parse(&response)?, &mut events)?;
            println!("parsed {} events", events.len());

            // read RFC 3339 timestamp from filesystem, defaulting to 28 days before current iteration
            println!("reading leaderboard timestamp from filesystem");
            let timestamp = File::open(format!("timestamp_{year}_{leaderboard}")).ok().and_then(|mut f|
            {
                buffer.clear();
                f.read_to_string(&mut buffer).ok()
                 .and_then(|_| DateTime::parse_from_rfc3339(buffer.trim()).ok())
                 .map(|dt| dt.with_timezone(&Utc))
            })
            .unwrap_or_else(|| { println!("timestamp read failed, defaulting to 28 days ago"); next - Duration::days(28) });
            println!("obtained timestamp {timestamp}");

            // send a webhook for each event that took place after the latest timestamp, up to the start of this iteration
            for e in events.iter().skip_while(|e| e.timestamp <= timestamp).take_while(|e| e.timestamp < next)
            {
                send_webhook(webhook, client, &format!("{e}"))?;
                println!("updating timestamp to {}", e.timestamp);
                std::fs::write(format!("timestamp_{year}_{leaderboard}"), e.timestamp.to_rfc3339())?;
            }

            // check if an AoC year is currently live
            if year == next.year()
            {
                let day    = next.day();
                let puzzle = puzzle_unlock(year, day);

                if prev < puzzle && puzzle <= next
                {
                    // announce a new AoC year
                    if day == 1
                    {
                        send_webhook(webhook, client, &format!(":christmas_tree: Advent of Code {year} is now live! :christmas_tree:"))?
                    }

                    // announce a new puzzle
                    if day <= 25
                    {
                        send_webhook(webhook, client, &format!(":christmas_tree: [{year}] Puzzle {day:02} is now available! :christmas_tree:"))?;
                    }

                    // anounce current leaderboard standings
                    let report = if events.is_empty() { "No scores yet: start programming!".to_string() } else { standings(&events) };
                    send_webhook(webhook, client, &format!(":christmas_tree: [{year}] Current Standings (Reciprocal Scoring) :christmas_tree:\n```{report}```"))?;
                }
            }
        }

        // roll over timestamps for next iteration
        prev = next;
        println!("iteration ended at {}", Utc::now());
    }
}

// puzzles unlock at 05:00 UTC each day from 1st to 25th December
fn puzzle_unlock(year : i32, day : u32) -> DateTime<Utc>
{
    Utc.with_ymd_and_hms(year, 12, day, 5, 0, 0).unwrap()
}

fn request_events(year : i32, leaderboard : &str, session : &str, client : &Client) -> Result<String, Box<dyn Error>>
{
    let url = format!("https://adventofcode.com/{year}/leaderboard/private/view/{leaderboard}.json");

    match client.get(&url).header("cookie", format!("session={session}")).send()
    {
        Ok(r)  => Ok(r.text()?),
        Err(e) => Err(Box::new(e))
    }
}

fn vectorise_events(json : &JsonValue, events : &mut Vec<Event>) -> Result<(), Box<dyn Error>>
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
                    timestamp: Utc.timestamp_opt(contents["get_star_ts"].as_i64().unwrap(), 0).unwrap(),
                    year:      json["event"].to_string().parse()?,
                    day:       day.parse()?,
                    star:      star.parse()?,
                    id:        Identifier { name: name.clone(), numeric: id.parse()? }
                });
            }
        }
    }

    events.sort_unstable();
    Ok(())
}

fn score_events(events : &[Event]) -> Vec<(&Identifier, BigRational)>
{
    // score histogram
    let mut hist : HashMap<&Identifier, BigRational> = HashMap::new();
    for e in events
    {
        *hist.entry(&e.id).or_insert_with(|| FromPrimitive::from_u8(0).unwrap()) += e.score();
    }

    // sort by score descending, then by Identifier ascending
    let mut scores = hist.into_iter().collect::<Vec<_>>();
    scores.sort_unstable_by_key(|(id, score)| (-score, *id));
    scores
}

fn standings(events : &[Event]) -> String
{
    // calculate maximum-length name for space padding
    let scores   = score_events(events);
    let max_name = scores.iter().map(|(id, _)| id.name.len()).max().unwrap_or(0);

    // generate standings report, with one line per participant
    let mut report = String::new();
    for (id, score) in scores
    {
        report.push_str(&format!("{}:", id.name));
        for _ in id.name.len() ..= max_name { report.push(' ') }
        report.push_str(&format!("{:>5.02}", score.to_f64().unwrap()));
        report.push('\n');
    }
    report
}

fn send_webhook(url : &str, client : &Client, text : &str) -> Result<(), Box<dyn Error>>
{
    println!("sending webhook: {:?}", text);
    let json = json::object!{ content: text };

    loop
    {
        let response = client.post(url)
                             .header("Content-Type", "application/json")
                             .body(json.to_string())
                             .send()?;

        match response.status()
        {
            StatusCode::NO_CONTENT        => break,
            StatusCode::TOO_MANY_REQUESTS =>
            {
                let retry = json::parse(&response.text()?)?["retry_after"].as_f32().unwrap_or(0.0);
                println!("rate-limited for {}s", retry);
                std::thread::sleep(std::time::Duration::from_millis((retry * 1000.0) as u64));
            },
            c => println!("unexpected status code {}", c)
        }

        println!("retrying");
    }

    Ok(())
}
