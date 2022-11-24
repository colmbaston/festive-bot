use json::JsonValue;
use reqwest::{ StatusCode, blocking::Client };
use chrono::{ Utc, Datelike, TimeZone, Duration, DurationRound };
use std::{ env, fs::File, io::Read, error::Error, fmt::{ Display, Formatter }, collections::HashMap };
use rational::Rational;

fn main() -> Result<(), Box<dyn Error>>
{
    let leaderboard = env::var("FESTIVE_BOT_LEADERBOARD")?;
    let session     = env::var("FESTIVE_BOT_SESSION")?;
    let webhook     = env::var("FESTIVE_BOT_WEBHOOK")?;

    let client = Client::new();

    /*
    if let Err(e) = update_loop(&leaderboard, &session, &webhook, &client)
    {
        let _ = send_webhook(&webhook, &client, ":christmas_tree: Festive Bot encountered an error and is exiting! :warning:");
        return Err(e)
    }
    */

    let mut events = Vec::new();
    let response   = request_events(2021, &leaderboard, &session, &client)?;
    vectorise_events(&json::parse(&response)?, &mut events)?;

    for (id, s) in score_events(&events)
    {
        println!("{}: {}", id.name, s);
    }

    Ok(())
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Event
{
    timestamp: u64,
    year:      u16,
    day:       u8,
    star:      u8,
    id:        Identifier
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Identifier
{
    numeric: u32,
    name:    String
}

impl Display for Event
{
    fn fmt(&self, f : &mut Formatter) -> std::fmt::Result
    {
        let (parts, star) = match self.star
        {
            1 => ("the first part", ":star:"),
            _ => ("both parts",     ":star: :star:"),
        };

        write!(f, ":christmas_tree: [{}] {} has completed {} of puzzle {:02} {}", self.year, self.id.name, parts, self.day, star)
    }
}

impl Event
{
    fn days_to_complete(&self) -> u16
    {
        let puzzle_release  = Utc.with_ymd_and_hms(self.year as i32, 12, self.day as u32, 5, 0, 0).unwrap();
        let puzzle_complete = Utc.timestamp_opt(self.timestamp as i64, 0).unwrap();

        (puzzle_complete - puzzle_release).num_days() as u16
    }

    // custom scoring based on the reciprocal of full days since the puzzle was released
    fn score(&self) -> Rational
    {
        Rational::new(1, 1 + self.days_to_complete())
    }
}

fn update_loop(leaderboard : &str, session : &str, webhook : &str, client : &Client) -> Result<(), Box<dyn Error>>
{
    // reusable buffers for efficiency
    let mut events = Vec::new();
    let mut buffer = String::new();

    // populate currently-live Advent of Code events
    let mut live = Vec::new();
    let mut now  = Utc::now();
    let year     = now.year();
    live.extend(2015 .. year);
    if now >= Utc.with_ymd_and_hms(year, 12, 1, 5, 0, 0).unwrap() { live.push(year) }

    loop
    {
        // send API requests only once every 15 minutes
        let delay = Duration::minutes(15);
        let next  = now.duration_trunc(delay)? + delay;
        println!("sleeping until {}", next);
        std::thread::sleep((next - now).to_std()?);
        println!("woke at {}", Utc::now());

        // check if new Advent of Code event has started since this function was first called
        let year  = now.year();
        let start = Utc.with_ymd_and_hms(year, 12, 1, 5, 0, 0).unwrap();
        if now < start && start <= next
        {
            live.push(year);
            let _ = send_webhook(webhook, client, &format!(":christmas_tree: Advent of Code {} is now live! :christmas_tree:", year));
        }

        for year in live.iter()
        {
            // send API request to the Advent of Code leaderboard, parse and vectorise the results
            println!("sending API request for year {}", year);
            let response = request_events(*year as u16, leaderboard, session, client)?;
            println!("parsing response");
            vectorise_events(&json::parse(&response)?, &mut events)?;
            println!("parsed {} events", events.len());

            // read the timestamp of the latest-reported event from the filesystem, or default to zero
            println!("reading timestamp from filesystem");
            let last_timestamp = File::open(format!("{}.txt", year)).ok().and_then(|mut f|
            {
                buffer.clear();
                f.read_to_string(&mut buffer).ok().and(buffer.trim_end().parse().ok())
            })
            .unwrap_or(0);

            // send a webhook for each event that took place after the latest timestamp, updating the timestamp each time
            for e in events.iter().skip_while(|e| e.timestamp <= last_timestamp)
            {
                send_webhook(webhook, client, &format!("{}", e))?;
                println!("updating timestamp on filesystem");
                std::fs::write(format!("{}.txt", year), format!("{}\n", e.timestamp).as_bytes())?;
            }
        }

        now = Utc::now();
        println!("finished at {}", now);
    }
}

fn request_events(year : u16, leaderboard : &str, session : &str, client : &Client) -> Result<String, Box<dyn Error>>
{
    let url = format!("https://adventofcode.com/{}/leaderboard/private/view/{}.json", year, leaderboard);

    match client.get(&url).header("cookie", format!("session={}", session)).send()
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
                    timestamp: contents["get_star_ts"].to_string().parse()?,
                    year:      json["event"].to_string().parse()?,
                    day:       day.parse()?,
                    star:      star.parse()?,
                    id:        Identifier { numeric: id.parse()?, name: name.clone() }
                });
            }
        }
    }

    events.sort_unstable();
    Ok(())
}

fn score_events(events : &[Event]) -> Vec<(&Identifier, Rational)>
{
    let mut scores = HashMap::new();
    for e in events
    {
        *scores.entry(&e.id).or_insert_with(Rational::zero) += e.score();
    }

    let mut scores = scores.into_iter().collect::<Vec<_>>();
    scores.sort_unstable_by_key(|&(id, s)| (-s, &id.name, id.numeric));
    scores
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
                let retry_ms = json::parse(&response.text()?)?["retry_after"].as_u64().unwrap_or(0);
                println!("rate-limited for {}ms", retry_ms);
                std::thread::sleep(std::time::Duration::from_millis(retry_ms));
            },
            c => println!("unexpected status code {}", c)
        }

        println!("retrying");
    }

    Ok(())
}
