use std::collections::HashMap;
use chrono::{ DateTime, Utc, TimeZone };
use reqwest::{ blocking::Client, StatusCode };
use num::{ FromPrimitive, ToPrimitive, rational::BigRational };
use crate::error::{ FestiveResult, FestiveError };

// puzzle completion events parsed from AoC API
// year and day fields match corresponding components of DateTime<Utc>
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct Event
{
    timestamp: DateTime<Utc>,
    year:      i32,
    day:       u32,
    star:      u8,
    id:        Identifier
}

// unique identifier for a participant on this leaderboard
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Identifier
{
    name:    String,
    numeric: u64
}

impl Event
{
    pub fn timestamp(&self) -> &DateTime<Utc>
    {
        &self.timestamp
    }

    // not using Display trait so FestiveResult can be returned
    pub fn fmt(&self) -> FestiveResult<String>
    {
        let (part, stars) = match self.star
        {
            1 => ("one", ":star:"),
            2 => ("two", ":star: :star:"),
            _ => return Err(FestiveError::Parse)
        };

        let score  = self.score()?;
        let plural = if score == num::one() { "" } else { "s" };
        Ok(format!(":christmas_tree: [{}] {} has completed puzzle {:02}, part {part}, scoring {score} point{plural}! {stars}", self.year, self.id.name, self.day))
    }

    // custom scoring based on the reciprocal of full days since the puzzle was released
    pub fn score(&self) -> FestiveResult<BigRational>
    {
        let days                = (self.timestamp - Self::puzzle_unlock(self.year, self.day)?).num_days();
        let ratio : BigRational = FromPrimitive::from_i64(1 + days).ok_or(FestiveError::Conv)?;
        Ok(ratio.recip())
    }

    // puzzles unlock at 05:00 UTC each day from 1st to 25th December
    // additionally used for daily standings announcements on the 26th to 31st December
    pub fn puzzle_unlock(year : i32, day : u32) -> FestiveResult<DateTime<Utc>>
    {
        Utc.with_ymd_and_hms(year, 12, day, 5, 0, 0).single().ok_or(FestiveError::Conv)
    }

    pub fn request(year : i32, leaderboard : &str, session : &str, client : &Client) -> FestiveResult<String>
    {
        let url = format!("https://adventofcode.com/{year}/leaderboard/private/view/{leaderboard}.json");

        // send HTTP request
        let response = client.get(&url)
                             .header("cookie", format!("session={session}"))
                             .send()
                             .map_err(|_| FestiveError::Http)?;

        match response.status()
        {
            // expected response, get the text from the payload
            StatusCode::OK => response.text().map_err(|_| FestiveError::Http),

            // AoC responds with INTERNAL_SERVER_ERROR when the session cookie is invalid
            StatusCode::INTERNAL_SERVER_ERROR =>
            {
                println!("the session cookie might have expired");
                Err(FestiveError::Http)
            }

            // unexpected status code
            _ => Err(FestiveError::Http)
        }
    }

    pub fn parse(response : &str, events : &mut Vec<Event>) -> FestiveResult<()>
    {
        // the response should be valid JSON
        let json = json::parse(response).map_err(|_| FestiveError::Parse)?;

        // iterate through the JSON, collating individual puzzle completion events
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
                        timestamp: Utc.timestamp_opt(contents["get_star_ts"].as_i64().ok_or(FestiveError::Parse)?, 0).single().ok_or(FestiveError::Conv)?,
                        year:      json["event"].to_string().parse().map_err(|_| FestiveError::Parse)?,
                        day:       day.parse().map_err(|_| FestiveError::Parse)?,
                        star:      star.parse().map_err(|_| FestiveError::Parse)?,
                        id:        Identifier { name: name.clone(), numeric: id.parse().map_err(|_| FestiveError::Parse)? }
                    });
                }
            }
        }

        // events are sorted chronologically
        events.sort_unstable();
        Ok(())
    }

    pub fn standings(events : &[Event]) -> FestiveResult<String>
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

        // calculate maximum-length name for space padding
        let max_name = scores.iter().map(|(id, _)| id.name.len()).max().unwrap_or(0);

        // generate standings report, with one line per participant
        let mut standings = String::new();
        for (id, score) in scores
        {
            standings.push_str(&format!("{}:", id.name));
            for _ in id.name.len() ..= max_name { standings.push(' ') }
            standings.push_str(&format!("{:>5.02}", score.to_f64().ok_or(FestiveError::Conv)?));
            standings.push('\n');
        }
        Ok(standings)
    }
}
