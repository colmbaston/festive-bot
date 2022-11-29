use reqwest::{ blocking::Client, StatusCode };
use crate::error::{ FestiveResult, FestiveError };

// handles for webhook URLs
#[derive(Clone, Copy, Debug)]
pub enum Webhook { Notify, Status }

impl Webhook
{
    // the environment variable associated with this webhook
    fn env_var(&self) -> &'static str
    {
        const NOTIFY : &str = "FESTIVE_BOT_NOTIFY";
        const STATUS : &str = "FESTIVE_BOT_STATUS";

        match self
        {
            Webhook::Notify => NOTIFY,
            Webhook::Status => STATUS
        }
    }

    pub fn send(payload : &str, webhook : Webhook, client : &Client) -> FestiveResult<()>
    {
        println!("webhook payload: {payload:?}");

        // only send HTTP request if webhook variable set
        match &std::env::var(webhook.env_var())
        {
            Err(_)  => println!("webhook variable {} fetch failed, not sending request", webhook.env_var()),
            Ok(url) =>
            {
                println!("webhook URL: {url}");
                if let Ok(url) = &std::env::var(webhook.env_var())
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
            }
        }

        Ok(())
    }
}
