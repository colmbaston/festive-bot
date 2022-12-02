# Festive Bot

A bot to track events occurring on a private Advent of Code (AoC) leaderboard, written in Rust.
Festive Bot reads the leaderboard data from https://adventofcode.com, parses the puzzle completion events, and reports updates that occur using a webhook HTTP URL.

The webhook HTTP requests conform to Discord's webhook API, and this is the only service Festive Bot is verified to support.
Other services' webhook APIs may be (partially) compatible, but I can make no guarantees.

## Usage

### Environment Variables

Environment variables `FESTIVE_BOT_LEADERBOARD` and `FESTIVE_BOT_SESSION` must be provided at runtime.
These are the ID of the private leaderboard to monitor, and a session cooike for an AoC account that has access to that leaderboard.

Optionally, environment variables `FESTIVE_BOT_NOTIFY` and `FESTIVE_BOT_STATUS` may also be provided.
These are HTTP URLs for webhooks, defining where puzzle completion notifications, and messages about the status of Festive Bot (including unrecoverable errors), repectively, are sent.
Both variables may contain the same URL, and if unset, no HTTP requests will be sent for the corresponding variable.

### Command-Line Arguments

By default, Festive Bot only report on puzzle completions the current year's AoC (and therefore only does anything useful during December).
Setting the `--all-years` flag allows reporting on puzzle completions for past AoC years as well, though the leaderboard standings for these years won't be posted.

You may set `--heartbeat` to send an additional message to the status webhook once an hour.
This can be useful when Festive Bot is running on a machine that you cannot easily monitor, giving some additional assurance that the bot is still alive.

### Cached Files

Per-year, per-leaderboard timestamp files (`timestamp_2015_123456` for year 2015 and leaderboard ID 123456) will will be cached in the current working directory.
Puzzle completions which occur before the corresponding timestamp won't be reported.
These files may be edited manually if desired: they should be UTF-8 encoded and conform to the RFC 3339 date and time standard.

## Custom Scoring

Since it is inconvenient to compete on the official AoC leaderboard in certain time zones, Festive Bot implements a custom scoring system.
Scores are assigned per-puzzle based on the reciprocal of the number of full days since the puzzle was released.

Each star is worth one point on the first day it's available, half a point on day two, a third on day three, and so on.
This gives a maximum score per-year equal to the number of stars, allows participants to schedule AoC at whatever time suits them, and ensures every puzzle completion awards a non-zero number of points.
