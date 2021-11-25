use json::JsonValue;
use reqwest::{ blocking::Client, StatusCode };
use std::{ error::Error, fs::File, io::{ Read, Write }, fmt::{ Display, Formatter }};

fn main() -> Result<(), Box<dyn Error>>
{
    // data required for the bot to run
    let year        = "2015";
    let session     = include_str!("../session.txt").trim_end();
    let leaderboard = include_str!("../leaderboard.txt").trim_end();
    let webhook     = include_str!("../webhook.txt").trim_end();

    // send API request to the Advent of Code leaderboard, parse and vectorise the results
    let leaderboard = format!("https://adventofcode.com/{}/leaderboard/private/view/{}.json", year, leaderboard);
    let text        = Client::new().get(&leaderboard).header("cookie", format!("session={}", session)).send()?.text()?;
    let json        = json::parse(&text)?;
    let events      = vectorise_events(&json);

    // read the timestamp of the latest-reported event from the filesystem, or default to zero
    let timestamp = File::open("timestamp.txt").ok().and_then(|mut f|
    {
        let mut s = String::new();
        f.read_to_string(&mut s).ok().and(s.trim_end().parse().ok())
    })
    .unwrap_or(0);

    // send a webhook for each event that took place after the latest timestamp, updating the timestamp each time
    for e in events.iter().filter(|e| e.timestamp > timestamp)
    {
        send_webhook(webhook, &format!("{}", e))?;
        File::create("timestamp.txt")?.write_all(format!("{}\n", e.timestamp).as_bytes())?;
    }

    Ok(())
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Event
{
    timestamp: u64,
    name:      String,
    day:       u8,
    star:      u8
}

impl Display for Event
{
    fn fmt(&self, f : &mut Formatter) -> std::fmt::Result
    {
        let part = if self.star == 1 { "the first part" } else { "both parts" };
        write!(f, ":star: {} has completed {} of puzzle {} :star:", self.name, part, self.day)
    }
}

fn vectorise_events(json : &JsonValue) -> Vec<Event>
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
                    timestamp: contents["get_star_ts"].to_string().parse().unwrap(),
                    name:      name.clone(),
                    day:       day.parse().unwrap(),
                    star:      star.parse().unwrap(),
                });
            }
        }
    }

    events.sort();
    events
}

fn send_webhook(url : &str, text : &str) -> Result<(), Box<dyn Error>>
{
    println!("sending webhook: {}", text);
    let json = json::object!{ content: text };

    loop
    {
        let response = Client::new().post(url)
                                    .header("Content-Type", "application/json")
                                    .body(json.to_string())
                                    .send()?;

        match response.status()
        {
            StatusCode::NO_CONTENT        => { println!("success"); break },
            StatusCode::TOO_MANY_REQUESTS =>
            {
                let retry_ms = json::parse(&response.text()?)?["retry_after"].as_u64().unwrap();
                println!("rate-limited for {}ms", retry_ms);
                std::thread::sleep(std::time::Duration::from_millis(retry_ms));
            },
            c => println!("unexpected status code {}", c)
        }

        println!("retrying");
    }

    Ok(())
}
