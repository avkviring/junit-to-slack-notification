FROM rust:1.81.0 AS builder
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY ./src ./src
RUN cargo build --release

FROM ubuntu:25.04
RUN apt-get update && apt install -y libssl3 && apt install -y ca-certificates
WORKDIR /usr/src/app
COPY --from=builder /usr/src/app/target/release/junit_to_slack_notification /junit_to_slack_notification
RUN chmod +x /junit_to_slack_notification
CMD ["/junit_to_slack_notification"]
