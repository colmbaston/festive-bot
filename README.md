# Festive Bot

A simple bot to track events occurring on a private Advent of Code leaderboard, written in Rust.
The bot reads the leaderboard data from https://adventofcode.com, parses the puzzle completion events, and pushes updates that occur using a webhook, for example to announce them in a Discord or Slack server.

The following environment variables should be provided at runtime:
* `FESTIVE_BOT_LEADERBOARD`: containing the ID of the private leaderboard to watch;
* `FESTIVE_BOT_SESSION`:     containing the session cookie of an Advent of Code account with access to that leaderboard;
* `FESTIVE_BOT_WEBHOOK`:     containing the webhook URL to push updates to.
