# Festive Bot

A bot to track events occurring on a private Advent of Code (AoC) leaderboard, written in Rust.
The bot reads the leaderboard data from https://adventofcode.com, parses the puzzle completion events, and pushes updates that occur using a webhook.

The webhook HTTP requests conform to Discord's webhook API, and this is the only service the bot is verified to support.
Other services' webhook APIs may be (partially) compatible, but I give no guarantees.

## Usage

### Environment Variables

Environment variables `FESTIVE_BOT_LEADERBOARD` and `FESTIVE_BOT_SESSION` must be provided at runtime.
These are the ID of the private leaderboard to monitor, and a session cooike for an AoC account that has access to that leaderboard.

Optionally, environment variables `FESTIVE_BOT_NOTIFY` and `FESTIVE_BOT_STATUS` may also be provided.
These are HTTP URLs for webhooks, defining where puzzle event notifications and messages about the bot's status (including unrecoverable errors), repectively, are sent.
Both variables may contain the same URL, and if unset, no HTTP requests will be sent for the corresponding variable.

### Command-Line Arguments

To restrict the bot to only report on puzzle completions from the current AoC year, set the `--current-year-only` command-line argument.
All other arguments are ignored.

### Cached Files

Per-year, per-leaderboard timestamp files (`timestamp_2015_123456` for year 2015 and leaderboard ID 123456) will will be cached in the bot's working directory.
Puzzle completions which occur before the corresponding timestamp won't be reported.
These files may be edited manually if desired: they should be UTF-8 encoded and conform to the RFC 3339 date and time standard.

## Custom Scoring

Since it is inconvenient to compete on the official AoC leaderboard in certain time zones, Festive Bot implements a custom scoring system.
Scores are assigned per-puzzle based on the reciprocal of the number of full days since the puzzle was released.

Each star is worth one point on the first day it's available, half a point on day two, a third on day three, and so on.
This gives a maximum score per-year equal to the number of stars, allows participants to schedule AoC at whatever time suits them, and ensures every puzzle completion awards a non-zero number of points.
