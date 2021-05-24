use teloxide::requests::{Request, Requester, RequesterExt};

use anyhow::anyhow;
use dotenv::dotenv;
use soup::prelude::*;
use std::env;

#[derive(Debug, Clone, PartialEq)]
struct UserCredentials {
    username: String,
    password: String,
}

#[derive(Debug, Clone, PartialEq)]
struct TournamentInformation {
    tournament_name: String,
    tournament_type: String,
    tournament_start_time: String,
}

impl UserCredentials {
    async fn authenticate(self, url: String) -> Result<TournamentInformation, anyhow::Error> {
        let client = reqwest::Client::new();
        let payload = [
            (String::from("username"), self.username),
            (String::from("password"), self.password),
        ];

        let response = client.post(url).form(&payload).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "fetching ag45.dots.org.ua failed with HTTP code: {}",
                response.status()
            ));
        };

        let website_html = response.text().await?;
        if !website_html.contains("Выйти") {
            return Err(anyhow!(
                "attempt to login was not succeed due to incorrect data"
            ));
        }

        let soup = Soup::new(&website_html);
        let parsed_website_html = soup.tag("td").class("pt").find_all();
        let all_tournaments_information =
            parsed_website_html.map(|a| a.display()).collect::<Vec<_>>();
        let last_tournament_information = all_tournaments_information[0]
            .split('>')
            .collect::<Vec<_>>();

        Ok(TournamentInformation {
            tournament_name: String::from(
                &last_tournament_information[2][..last_tournament_information[2].len() - 3],
            ),
            tournament_start_time: String::from(
                &last_tournament_information[14][..last_tournament_information[14].len() - 3],
            ),
            tournament_type: String::from(
                &last_tournament_information[9][..last_tournament_information[9].len() - 3],
            ),
        })
    }
}

#[tokio::main]
async fn main() {
    run().await;
}

async fn run() {
    dotenv().ok();
    teloxide::enable_logging!();
    log::info!("Starting Ag45Bot...");

    let bot = teloxide::Bot::from_env().auto_send();

    let operations_polling_interval = std::time::Duration::from_secs(10);
    let delay_between_failing_attempts = std::time::Duration::from_secs(3);

    let mut last_reported_tournament: Option<TournamentInformation> = None;

    let my_credentials = UserCredentials {
        username: String::from("10205"),
        password: String::from("WFN-CvX-ziT-v1D"),
    };

    loop {
        let lastest_tournament: TournamentInformation = match my_credentials
            .clone()
            .authenticate(String::from("https://ag45.dots.org.ua/login"))
            .await
        {
            Ok(tournament) => tournament,
            Err(err) => {
                log::info!(
                    "Failed to get latest tournament due to: {}. Retrying in {} seconds...",
                    err,
                    operations_polling_interval.as_secs_f64()
                );
                continue;
            }
        };

        match &last_reported_tournament {
            None => {
                last_reported_tournament = Some(lastest_tournament.clone());
                continue;
            }
            Some(last_reported_tournament) if last_reported_tournament == &lastest_tournament => {
                continue;
            }
            Some(_) => {}
        }

        let telegram_text = format!(
            "На сайте появился новый турнир!\nНазвание: {}\nВремя начала: {}\nТип: {}",
            lastest_tournament.tournament_name,
            lastest_tournament.tournament_start_time,
            lastest_tournament.tournament_type,
        );
        while let Err(err) = bot.send_message(455974403, &telegram_text).send().await {
            log::info!(
                "Failed to sent Telegram notification due to: {}. Retrying in {} seconds...",
                err,
                delay_between_failing_attempts.as_secs_f64()
            );
            tokio::time::sleep(delay_between_failing_attempts).await;
        }

        last_reported_tournament = Some(lastest_tournament);

        tokio::time::sleep(operations_polling_interval).await;
    }
}
