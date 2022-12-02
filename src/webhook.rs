use reqwest::{ blocking::{ Client, multipart::{ Form, Part }}, StatusCode };
use crate::error::{ FestiveResult, FestiveError };

// handles for webhook URLs
#[derive(Clone, Copy, Debug)]
pub enum Webhook { Notify, Status }

impl Webhook
{
    // the environment variable associated with this webhook
    fn env_var(&self) -> &'static str
    {
        match self
        {
            Webhook::Notify => "FESTIVE_BOT_NOTIFY",
            Webhook::Status => "FESTIVE_BOT_STATUS"
        }
    }

    // written for Discord's webhook API
    // may work partially for other services, but only verified for Discord
    pub fn send(content : &str, files : &[(&str, &[u8])], webhook : Webhook, client : &Client) -> FestiveResult<()>
    {
        println!("webhook content:    {content:?}");
        println!("webhook file count: {}", files.len());

        // only send HTTP request if webhook variable set
        match &std::env::var(webhook.env_var())
        {
            Err(_)  => println!("webhook variable {} fetch failed, not sending request", webhook.env_var()),
            Ok(url) =>
            {
                println!("webhook URL: {url}");

                loop
                {
                    // build multi-part form with text content and files
                    let mut form = Form::new().text("content", content.to_string());
                    for (ix, (name, data)) in files.iter().enumerate()
                    {
                        form = form.part(format!("files[{ix}]"), Part::bytes(data.to_vec()).file_name(name.to_string()));
                    }

                    // send the request
                    let response = client.post(url)
                                         .header("wait", "true")
                                         .multipart(form)
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
                        c =>
                        {
                            println!("unexpected status code: {c}");
                            return Err(FestiveError::Http)
                        }
                    }

                    println!("retrying");
                }
            }
        }

        Ok(())
    }
}
