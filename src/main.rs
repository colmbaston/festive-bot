use json::JsonValue;
use reqwest::
{
    StatusCode,
    blocking::Client
};
use std::
{
    fs::File,
    error::Error,
    io::{ Read, Write },
    fmt::{ Display, Formatter },
    time::{ SystemTime, Duration }
};

fn main() -> Result<(), Box<dyn Error>>
{
    let session     = include_str!("../session.txt").trim_end();
    let leaderboard = include_str!("../leaderboard.txt").trim_end();
    let webhook     = include_str!("../webhook.txt").trim_end();

    std::thread::spawn(|| aoc_2021(webhook));
    update_loop(session, leaderboard, webhook)
}

fn unix_millis() -> u128
{
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis()
}

fn sleep_until(timestamp : u64) -> bool
{
    let sleep_ms = (timestamp as u128 * 1000).saturating_sub(unix_millis());
    if sleep_ms > 0
    {
        std::thread::sleep(Duration::from_millis(sleep_ms as u64));
        return true
    }
    false
}

fn aoc_2021(webhook : &str)
{
    // UNIX timestamp for 2021/12/01 at 05:00 UTC
    if sleep_until(1_638_334_800)
    {
        let _ = send_webhook(webhook, ":christmas_tree: Advent of Code 2021 is now live! :christmas_tree:");
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Event
{
    timestamp: u64,
    name:      String,
    year:      u16,
    day:       u8,
    star:      u8
}

impl Display for Event
{
    fn fmt(&self, f : &mut Formatter) -> std::fmt::Result
    {
        let (parts, star) = match self.star
        {
            1 => ("the first part", ":star:"),
            _ => ("both parts",     ":star: :star:")
        };

        write!(f, ":christmas_tree: [{}] {} has completed {} of day {:02}: {}", self.year, self.name, parts, self.day, star)
    }
}

fn update_loop(session : &str, leaderboard : &str, webhook : &str) -> Result<(), Box<dyn Error>>
{
    loop
    {
        // send API requests only once every 15 minutes
        const DELAY : u128 = 1000 * 60 * 15;
        let unix = unix_millis();
        println!("sleeping");
        sleep_until(((unix + DELAY - unix % DELAY) / 1000) as u64);

        for year in 2015 ..= 2021
        {
            // send API request to the Advent of Code leaderboard, parse and vectorise the results
            println!("sending API request for year {}", year);
            let leaderboard = format!("https://adventofcode.com/{}/leaderboard/private/view/{}.json", year, leaderboard);
            let text        = Client::new().get(&leaderboard).header("cookie", format!("session={}", session)).send()?.text()?;
            println!("parsing response");
            let events      = vectorise_events(&json::parse(&text)?)?;

            // read the timestamp of the latest-reported event from the filesystem, or default to zero
            println!("reading timestamp");
            let last_timestamp = File::open(format!("{}.txt", year)).ok().and_then(|mut f|
            {
                let mut s = String::new();
                f.read_to_string(&mut s).ok().and(s.trim_end().parse().ok())
            })
            .unwrap_or(0);

            // send a webhook for each event that took place after the latest timestamp, updating the timestamp each time
            for e in events.iter().skip_while(|e| e.timestamp <= last_timestamp)
            {
                send_webhook(webhook, &format!("{}", e))?;
                println!("writing timestamp");
                File::create(format!("{}.txt", year))?.write_all(format!("{}\n", e.timestamp).as_bytes())?;
            }
        }
    }
}

fn vectorise_events(json : &JsonValue) -> Result<Vec<Event>, Box<dyn Error>>
{
    let mut events = Vec::new();

    for (_, member) in json["members"].entries()
    {
        let name = member["name"].to_string();

        for (day, stars) in member["completion_day_level"].entries()
        {
            for (star, contents) in stars.entries()
            {
                events.push(Event
                {
                    timestamp: contents["get_star_ts"].to_string().parse()?,
                    name:      name.clone(),
                    year:      json["event"].to_string().parse()?,
                    day:       day.parse()?,
                    star:      star.parse()?
                });
            }
        }
    }

    events.sort_unstable();
    Ok(events)
}

fn send_webhook(url : &str, text : &str) -> Result<(), Box<dyn Error>>
{
    println!("sending webhook: {:?}", text);
    let json = json::object!{ content: text };

    loop
    {
        let response = Client::new().post(url)
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
