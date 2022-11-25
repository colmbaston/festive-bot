# Festive Bot

A bot to track events occurring on a private Advent of Code (AoC) leaderboard, written in Rust.
The bot reads the leaderboard data from https://adventofcode.com, parses the puzzle completion events, and pushes updates that occur using a webhook, for example to announce them in a Discord server.

A per-year, per-leaderboard timestamp file (`timestamp_2015_123456` for year 2015 and leaderboard ID 123456) will will be written to the working directory to remember which puzzle events have already been reported.
These files may be edited manually if desired: they should be UTF-8 encoded, and conform to the RFC 3339 date and time standard.

Since it is inconvenient to compete on the official AoC leaderboard in certain time zones, Festive Bot implements a custom scoring system.
Scores are assigned per-puzzle based on the reciprocal of the number of full days since the puzzle was released.
For example, completing a puzzle on the day of release awards 1 point, then 1/2 points the next day, then 1/3 points, and so on.
This gives a maximum score per-year equal to the number of puzzles, allows a participant to fit schedule AoC at whatever time suits them, and ensures every puzzle completion awards a non-zero number of points.

The following environment variables should be provided at runtime:
* `FESTIVE_BOT_LEADERBOARD`: the private leaderboard ID to monitor;
* `FESTIVE_BOT_SESSION`:     the session cookie of an Advent of Code account with access to that leaderboard;
* `FESTIVE_BOT_WEBHOOK`:     the webhook URL that updates will be pushed to.
