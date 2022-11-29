# Festive Bot

A bot to track events occurring on a private Advent of Code (AoC) leaderboard, written in Rust.
The bot reads the leaderboard data from https://adventofcode.com, parses the puzzle completion events, and pushes updates that occur using a webhook, for example to announce them in a Discord server.

## Usage

The environment variables `FESTIVE_BOT_LEADERBOARD` and `FESTIVE_BOT_SESSION` must be provided at runtime.
These are the ID of the private leaderboard to monitor, and a session cooike for an AoC account that has access to that leaderboard.

Optionally, environment variables `FESTIVE_BOT_NOTIFY` and `FESTIVE_BOT_STATUS` may also be provided.
These are HTTP URLs for webhooks, defining where puzzle event notifications and messages about the bot's status (including unrecoverable errors), repectively, are sent.
Both variables may contain the same URL, and if unset, no HTTP requests will be sent for the corresponding variable.
These variables may be modified while the bot is running as they are fetched anew each time a webhook HTTP request is sent.

Per-year, per-leaderboard timestamp files (`timestamp_2015_123456` for year 2015 and leaderboard ID 123456) will will be written to the working directory.
Puzzle events which occurred before this timestamp won't be reported.
These files may be edited manually if desired: they should be UTF-8 encoded and conform to the RFC 3339 date and time standard.

## Scoring

Since it is inconvenient to compete on the official AoC leaderboard in certain time zones, Festive Bot implements a custom scoring system.
Scores are assigned per-puzzle based on the reciprocal of the number of full days since the puzzle was released.
For example, completing a puzzle on the day of release awards 1 point, then $`\frac{1}{2}`$ points the next day, then $`\frac{1}{3}`$ points, and so on.
This gives a maximum score per-year equal to the number of puzzles, allows participants to schedule AoC at whatever time suits them, and ensures every puzzle completion awards a non-zero number of points.
