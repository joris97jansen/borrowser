use reqwest::{blocking::Client, Error};

pub fn fetch_text(url: &str) -> Result<String, Error> {
    let client = Client::builder()
        .user_agent("Borrowser/0.1 (+htps://example.invalid)")
        .build()?;

    let resp = client.get(url).send()?.error_for_status()?;
    Ok(resp.text()?)
}
