use anyhow::Context;
use junit_parser::{TestCase, TestStatus, TestSuite, TestSuites};
use serde::Serialize;
use std::{env, fs};

#[derive(Serialize)]
struct SlackMessage {
    text: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1).map(|s| s.as_str()).unwrap_or("junit.xml");
    let webhook_url =
        env::var("SLACK_WEBHOOK_URL").context("SLACK_WEBHOOK_URL environment variable not set")?;

    let xml_content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file {}", file_path))?;
    let suites = junit_parser::from_reader(xml_content.as_bytes())?;

    let mut failed_tests = vec![];
    collect_failed_tests(&suites.suites, &mut failed_tests);
    if failed_tests.is_empty() {
        println!("All tests passed successfully!");
        return Ok(());
    }

    let message = format_slack_message(&failed_tests);
    send_slack_message(&message, &webhook_url).await?;
    Ok(())
}

fn collect_failed_tests(test_suites: &[TestSuite], result: &mut Vec<TestCase>) {
    for suite in test_suites {
        collect_failed_tests(&suite.suites, result);
        for case in &suite.cases {
            if has_failures(case) {
                result.push(case.clone());
            }
        }
    }
}

fn has_failures(case: &TestCase) -> bool {
    match case.status {
        TestStatus::Success => false,
        TestStatus::Error(_) => true,
        TestStatus::Failure(_) => true,
        TestStatus::Skipped(_) => false,
    }
}

fn format_slack_message(failed_cases: &[TestCase]) -> String {
    let title = env::var("SLACK_MESSAGE_TITLE").unwrap_or_else(|_| "Test Results".to_string());
    let mut message = format!(":x: *{}*\n\n", title);
    for case in failed_cases {
        append_case_info(&mut message, case);
    }
    message
}

fn append_case_info(message: &mut String, case: &TestCase) {
    message.push_str(&format!("-`{}`\n", case.name.to_string()));
}

async fn send_slack_message(message: &str, webhook_url: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .post(webhook_url)
        .json(&SlackMessage {
            text: message.to_owned(),
        })
        .send()
        .await
        .context("Failed to send message to Slack")?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        anyhow::bail!("Slack API error: {}", error_text);
    }

    println!("Results sent to Slack");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use junit_parser::{TestCase, TestError, TestFailure, TestSkipped, TestStatus, TestSuite, TestSuites};
    use mockito::Server;

    #[test]
    fn test_has_failures() {
        let success_case = TestCase {
            name: "test_success".to_string(),
            classname: Some("com.example.TestClass".to_string()),
            status: TestStatus::Success,
            time: 1.0,
            ..Default::default()
        };
        assert!(!has_failures(&success_case));

        let error_case = TestCase {
            name: "test_error".to_string(),
            classname: Some("com.example.TestClass".to_string()),
            status: TestStatus::Error(TestError{
                message: "".to_string(),
                text: "".to_string(),
                error_type: "".to_string(),
            }),
            time: 1.0,
            ..Default::default()
        };
        assert!(has_failures(&error_case));

        let failure_case = TestCase {
            name: "test_failure".to_string(),
            classname: Some("com.example.TestClass".to_string()),
            status: TestStatus::Failure(TestFailure {
                message: "".to_string(),
                text: "".to_string(),
                failure_type: "".to_string(),
            }),
            time: 1.0,
            ..Default::default()
        };
        assert!(has_failures(&failure_case));

        let skipped_case = TestCase {
            name: "test_skipped".to_string(),
            classname: Some("com.example.TestClass".to_string()),
            status: TestStatus::Skipped(TestSkipped {
                message: "".to_string(),
                text: "".to_string(),
                skipped_type: "".to_string(),
            }),
            time: 1.0,
            ..Default::default()
        };
        assert!(!has_failures(&skipped_case));
    }



    #[test]
    fn test_append_case_info() {
        let case = TestCase {
            name: "test_method".to_string(),
            classname: Some("com.example.TestClass".to_string()),
            status: TestStatus::Failure(TestFailure {
                message: "".to_string(),
                text: "".to_string(),
                failure_type: "".to_string(),
            }),
            time: 2.5,
            ..Default::default()
        };

        let mut message = String::new();
        append_case_info(&mut message, &case);
        assert!(message.contains(&case.name));
    }

    #[tokio::test]
    async fn test_send_slack_message_success() {
        let mut server = Server::new_async().await;
        let mock_url = server.url();

        let mock = server
            .mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .create();

        let result = send_slack_message("test message", &mock_url).await;

        assert!(result.is_ok());
        mock.assert();
    }

    #[tokio::test]
    async fn test_send_slack_message_failure() {
        let mut server = Server::new_async().await;
        let mock_url = server.url();

        let mock = server
            .mock("POST", "/")
            .with_status(500)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error":"internal server error"}"#)
            .create();
        let result = send_slack_message("test message", &mock_url).await;

        assert!(result.is_err());
        mock.assert();
    }
}
