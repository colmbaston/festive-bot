# Festive Bot

A simple bot to track events occurring on a private Advent of Code leaderboard, written in Rust.
The bot reads the leaderboard data from https://adventofcode.com, parses the puzzle completion events, and pushes updates that occur using a webhook, for example to announce them in a Discord or Slack server.

To compile, the following files must be present in the root directory:
* `leaderboard.txt`: containing the ID of the private leaderboard to watch;
* `session.txt`: containing the session cookie of an Advent of Code account that is able to see that leaderboard;
* `webhook.txt`: containing the webhook URL to push updates to.
