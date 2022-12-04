use std::{ collections::HashMap, fmt::Write };
use json::JsonValue;
use chrono::{ DateTime, Utc, FixedOffset, TimeZone, Duration, DurationRound };
use reqwest::{ blocking::Client, StatusCode };
use num::{ FromPrimitive, ToPrimitive, rational::BigRational };
use crate::error::{ FestiveResult, FestiveError };

// puzzle completion events parsed from AoC API
// year and day fields match corresponding components of DateTime<Utc>
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Event
{
    timestamp: DateTime<Utc>,
    year:      i32,
    day:       u32,
    star:      u8,
    id:        Identifier
}

// unique identifier for a participant on this leaderboard
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

    // use UTC timestamps, but truncate centered on UTC-05:00 (EST), as this is when puzzles unlock
    pub fn trunc_ts(ts : &DateTime<Utc>, dur : Duration) -> FestiveResult<DateTime<Utc>>
    {
        let est    = FixedOffset::west_opt(5 * 3600).ok_or(FestiveError::Conv)?;
        let est_ts = ts.with_timezone(&est);

        est_ts.duration_trunc(dur)
              .map(|dt| dt.with_timezone(&Utc))
              .map_err(|_| FestiveError::Conv)
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
        let response = client.get(url)
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
            let name = match &member["name"]
            {
                // anonymous users appear with null names in the AoC API
                JsonValue::Null         => format!("anonymous user #{id}"),
                JsonValue::Short(name)  => name.to_string(),
                JsonValue::String(name) => name.clone(),
                _                       => return Err(FestiveError::Parse)
            };

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

        // sort by score descending, then by Identifier ascending, and group distinct scores
        let mut scores = hist.into_iter().collect::<Vec<_>>();
        scores.sort_unstable_by_key(|(id, score)| (-score, *id));
        let distinct = scores.group_by(|a, b| a.1 == b.1).collect::<Vec<_>>();

        // calculate width for positions
        // the width of the maximum position to be displayed, plus one for ')'
        let width_pos = 2 + (1 + distinct.iter()
                                         .rev()
                                         .skip(1)
                                         .map(|grp| grp.len())
                                         .sum::<usize>()).ilog10() as usize;

        // calculate width for names
        // the length of the longest name, plus one for ':'
        let width_name = 1 + scores.iter()
                                   .map(|(id, _)| id.name.len()).max()
                                   .unwrap_or(0);

        // calculate width for scores
        // the width of the maximum score, formatted to two decimal places
        let width_score = scores.iter()
                                .map(|(_,  s)| s)
                                .max()
                                .map(|s| 4 + s.to_f64().unwrap_or(0.0).log10().floor() as usize)
                                .unwrap_or(0);

        // generate standings report, with one line per participant
        let mut report = String::new();
        for (pos, grp) in distinct.into_iter().scan(1, |pos, grp| { let old = *pos; *pos += grp.len(); Some((old, grp)) })
        {
            for (ix, (id, score)) in grp.iter().enumerate()
            {
                writeln!(&mut report, "{:>width_pos$} {:<width_name$} {:>width_score$.02}",
                                      if ix == 0 { format!("{pos})") } else { String::new() },
                                      format!("{}:", id.name),
                                      score.to_f64().unwrap()).map_err(|_| FestiveError::Conv)?;
            }
        }
        Ok(report)
    }
}
