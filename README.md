# Festive Bot

A bot to track events occurring on a private Advent of Code (AoC) leaderboard, written in Rust.
Festive Bot reads the leaderboard data from https://adventofcode.com, parses the puzzle completion events, and reports updates that occur using a webhook HTTP URL.

The webhook HTTP requests conform to Discord's webhook API, and this is the only service Festive Bot is verified to support.
Other services' webhook APIs may be (partially) compatible, but I make no guarantees.

## Usage

### Environment Variables

Environment variables `FESTIVE_BOT_LEADERBOARD` and `FESTIVE_BOT_SESSION` must be provided at runtime.
These are the ID of the private leaderboard to monitor, and a session cookie for an AoC account that has access to that leaderboard.

Optionally, environment variables `FESTIVE_BOT_NOTIFY` and `FESTIVE_BOT_STATUS` may also be provided.
These are HTTP URLs for webhooks, defining where puzzle completion notifications, and messages about the status of Festive Bot (including unrecoverable errors), respectively, are sent.
Both variables may contain the same URL, and if unset, no HTTP requests will be sent for the corresponding variable.

### Command-Line Options

```
Usage: festive-bot [--all-years] [--period mins] [--standings mins] [--heartbeat mins]
```

By default, Festive Bot will only report on puzzle completions the current year's AoC and therefore only does anything useful during December.
Setting the `--all-years` flag allows reporting on puzzle completions for past AoC years as well, though the leaderboard standings for these years won't be announced.

Festive Bot runs in a cycle, fetching events from the AoC leaderboard, sending webhooks, then sleeping until the beginning of the next iteration.
The default iteration period is one hour, and can be modified by the `--period mins` option, specifying the period in minutes.
The minimum accepted value for the `mins` parameter is 15 minutes, limited to avoid requests being sent to the AoC API too frequently.
To ensure an iteration begins at 05:00 UTC each day (the moment new puzzles unlock), `mins` must divide evenly into 1440 (one day).

During December, Festive Bot will periodically announce the current leaderboard standings.
The default interval between standings announcements is one day, and can be modified by the `--standings mins` option, specifying the interval between announcements in minutes.
The maximum value for the `mins` parameter is 10080 (one week), and it must be a multiple of the iteration period.

You may optionally send heartbeat status messages to the status webhook, which can be useful when Festive Bot is running on a machine that you cannot easily monitor.
By default, no heartbeat messages are sent, but they can be enabled by the `--heartbeat mins` option, specifying the interval between heartbeats in minutes.
The maximum value for the `mins` parameter is 10080 (one week), and it must be a multiple of the iteration period.

### Cached Files

Per-year, per-leaderboard timestamp files (`timestamp_2015_123456` for year 2015 and leaderboard ID 123456) will will be cached to Festive Bot's working directory.
Puzzle completions which occur before the corresponding timestamp won't be reported.
These files may be edited manually if desired; they should be UTF-8 encoded and conform to the RFC 3339 date and time standard.

## Custom Scoring

Since it is inconvenient to compete on the official AoC leaderboard in certain time zones, Festive Bot implements a custom scoring system.
Scores are assigned per-puzzle based on the reciprocal of the number of full 24-hour periods since the moment the puzzle was released.

Each star is worth one point on the first day it's available, half a point on day two, a third on day three, and so on.
This gives a maximum score per-year equal to the number of stars, allows participants to schedule AoC at whatever time is convenient for them, and ensures every puzzle completion awards a non-zero number of points.
