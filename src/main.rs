use json::JsonValue;
use rational::Rational;
use reqwest::{ StatusCode, blocking::Client };
use chrono::{ Utc, DateTime, Datelike, TimeZone, Duration, DurationRound };
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
    numeric: u32,
    name:    String
}

impl Display for Event
{
    fn fmt(&self, f : &mut Formatter) -> std::fmt::Result
    {
        let (parts, stars) = match self.star
        {
            1 => ("the first part", ":star:"),
            2 => ("both parts",     ":star: :star:"),
            _ => panic!("cannot display star {}", self.star)
        };

        let  score          = self.score();
        let (score, plural) = if score == Rational::one() { ("1".to_string(), "") } else { (format!("{score}"), "s") };

        write!(f, ":christmas_tree: [{}] {} has completed {parts} of puzzle {:02}, scoring {score} point{plural} {stars}", self.year, self.id.name, self.day)
    }
}

impl Event
{
    fn days_to_complete(&self) -> i64
    {
        let puzzle_release  = Utc.with_ymd_and_hms(self.year as i32, 12, self.day as u32, 5, 0, 0).unwrap();

        (self.timestamp - puzzle_release).num_days()
    }

    // custom scoring based on the reciprocal of full days since the puzzle was released
    fn score(&self) -> Rational
    {
        Rational::new(1, 1 + self.days_to_complete())
    }
}

fn update_loop(leaderboard : &str, session : &str, webhook : &str, client : &Client) -> Result<(), Box<dyn Error>>
{
    println!("initialising");

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

        // use truncated now rather than fresh Utc::now() in case sleep lasts longer than expected
        now = Utc::now();
        println!("finished at {now}");
        now = now.duration_trunc(delay)?;

        // sleep until next cycle
        let next = now + delay;
        println!("sleeping until {next}");
        println!();
        std::thread::sleep((next - Utc::now()).to_std()?);
        println!("woke at {}", Utc::now());

        // check if new Advent of Code year has started since last cycle
        let year  = now.year();
        let start = Utc.with_ymd_and_hms(year, 12, 1, 5, 0, 0).unwrap();
        if now < start && start <= next
        {
            live.push(year);
            let _ = send_webhook(webhook, client, &format!(":christmas_tree: Advent of Code {year} is now live! :christmas_tree:"));
        }
        now = next;

        // read ISO 8601 timestamp from filesystem for this leaderboard, defaulting to 28 days ago
        println!("reading leaderboard timestamp from filesystem");
        let timestamp = File::open(format!("timestamp_{leaderboard}")).ok().and_then(|mut f|
        {
            buffer.clear();
            f.read_to_string(&mut buffer).ok()
             .and_then(|_| DateTime::parse_from_rfc3339(buffer.trim()).ok())
             .map(|dt| dt.with_timezone(&Utc))
        })
        .unwrap_or_else(|| { println!("timestamp read failed, defaulting to 28 days ago"); now - Duration::days(28) });
        println!("timestamp: {timestamp}");

        for &year in &live
        {
            // send API request to the Advent of Code leaderboard, parse and vectorise the results
            println!("sending API request for year {year}");
            let response = request_events(year, leaderboard, session, client)?;
            println!("parsing response");
            vectorise_events(&json::parse(&response)?, &mut events)?;
            println!("parsed {} events", events.len());

            // send a webhook for each event that took place after the latest timestamp, updating the timestamp each time
            for e in events.iter().skip_while(|e| e.timestamp <= timestamp)
            {
                send_webhook(webhook, client, &format!("{e}"))?;
                println!("updating timestamp to {}", e.timestamp);
                std::fs::write(format!("timestamp_{leaderboard}"), e.timestamp.to_rfc3339())?;
            }
        }
    }
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
