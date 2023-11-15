# High Budget Control Application

This is a very high budget control application, for synchronizing inputs over the internet. This program allows for players to hit their first note in a rhythm game chart at the same time. This allows for visual sync, which allows for stream syncing to make sense.

The desktop app currently only supports UNBEATABLE [white label] and its default Interact button, the `F` key.

## Quick Start

### As the host
The host is the person who will trigger the competitor's HBCA applications to press the F key, thus entering into the chart. To get started:

1. Go to https://hbca.pages.dev (this guarantees you'll be running the latest version).

2. Uncheck the "Is Client" checkbox. This is required if you're a host.

3. Give your peer ID to all competitors.
    > This technically exposes your IP, as it does with all competitors that connect to you. This is because this is a peer-to-peer app, and you should connect only to people you trust.

4. Once everyone's connected, and they've put the cassette into the player in-game, press the Start Countdown button.
    > Note the delay of when it finishes doing the countdown on your side, and when it'll visually appear. If you're using the restreaming servers via RTMP or SRT, the delay will be pretty large, so don't do a countdown in voice chat in production!

### As a competitor
1. Download HBCA from [the Releases section](https://github.com/some1chan/hbca/releases/), and install with either the .exe or .msi.
    > Antiviruses flag .exe installers commonly due to its behavior looking fairly similar to a virus. This application is also not frequently downloaded, which may also make an antivirus more suspicious. If you have problems with the .exe, try the .msi.
    > 
    > If for whatever reason you're not comfortable with installing the application, all the code's open source and viewable, and built via GitHub Actions. If it's still a problem, go to https://hbca.pages.dev/ instead, and make sure you time your inputs correctly to the countdown. 

2. Once installed and running, ask for host's peer ID. Then put it under **Peer to Connect To** and press Connect.

3. Figure out what song you need to play in UNBEATABLE, then put the cassette into the player. Don't enter the chart!
    > Doing this is important, as one more F key-press will make you enter in the chart, which is what we need to sync your stream up properly.

4. Get ready for the audio countdown. There will be five beats, and on the fifth, you will enter the chart.
