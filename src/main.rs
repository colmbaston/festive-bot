use json::JsonValue;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>>
{
    let year        = "2015";
    let session     = include_str!("../session.txt").trim_end();
    let leaderboard = include_str!("../leaderboard.txt").trim_end();

    let url  = format!("https://adventofcode.com/{}/leaderboard/private/view/{}.json", year, leaderboard);
    let json = json::parse(&reqwest::blocking::Client::new().get(&url)
                                                            .header("cookie", format!("session={}", session))
                                                            .send()?
                                                            .text()?)?;

    let events = vectorise_events(&json);

    for e in events.iter()
    {
        println!("{:?}", e);
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Event
{
    timestamp: u64,
    name:      String,
    day:       u8,
    star:      u8
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
