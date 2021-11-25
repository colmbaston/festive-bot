use std::error::Error;

fn main() -> Result<(), Box<dyn Error>>
{
    let year        = "2015";
    let session     = include_str!("../session.txt").trim_end();
    let leaderboard = include_str!("../leaderboard.txt").trim_end();

    let client   = reqwest::blocking::Client::new();
    let response = client.get(format!("https://adventofcode.com/{}/leaderboard/private/view/{}.json", year, leaderboard))
                         .header("cookie", format!("session={}", session))
                         .send()?;

    println!("{}", response.text()?);

    Ok(())
}
