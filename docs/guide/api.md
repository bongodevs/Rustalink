# REST API

Rustalink implements the Lavalink v4 API. Below are the supported endpoints and their raw responses.

## GET /v4/info

Get basic server information.

**Example request:**
```bash
curl -X GET -H "Authorization: youshallnotpass" "http://localhost:2333/v4/info"
```

<details>
<summary><b>Raw API response (Expand)</b></summary>

```json
{
  "version": {
    "semver": "1.0.5",
    "major": 1,
    "minor": 0,
    "patch": 5,
    "preRelease": null
  },
  "buildTime": 1773050806122,
  "git": {
    "branch": "dev",
    "commit": "802.....",
    "commitTime": 1773050774000
  },
  "jvm": "Rust",
  "lavaplayer": "symphonia",
  "sourceManagers": [
    "youtube",
    "spotify",
    "jiosaavn",
    "gaana",
    "tidal",
    "audiomack",
    "shazam",
    "mixcloud",
    "bandcamp",
    "reddit",
    "audius",
    "netease",
    "http",
    "local"
  ],
  "filters": [
    "volume",
    "equalizer",
    "karaoke",
    "timescale",
    "tremolo",
    "vibrato",
    "distortion",
    "rotation",
    "channelMix",
    "lowPass",
    "echo",
    "highPass",
    "normalization",
    "chorus",
    "compressor",
    "flanger",
    "phaser",
    "phonograph",
    "reverb",
    "spatial",
    "pluginFilters"
  ],
  "plugins": []
}
```
</details>

## GET /v4/stats

Get server statistics.

**Example request:**
```bash
curl -X GET -H "Authorization: youshallnotpass" "http://localhost:2333/v4/stats"
```

<details>
<summary><b>Raw API response (Expand)</b></summary>

```json
{
  "players": 0,
  "playingPlayers": 0,
  "uptime": 298962,
  "memory": {
    "free": 19219042304,
    "used": 49045504,
    "allocated": 49045504,
    "reservable": 32953487360
  },
  "cpu": {
    "cores": 8,
    "systemLoad": 0.31881649017333985,
    "lavalinkLoad": 0.00006302547175437213
  }
}
```
</details>

## GET /v4/routeplanner/status

Get routeplanner status.

**Example request:**
```bash
curl -X GET -H "Authorization: youshallnotpass" "http://localhost:2333/v4/routeplanner/status"
```

<details>
<summary><b>Raw API response (Expand)</b></summary>

```json

```
</details>

## GET /v4/loadtracks?identifier=ytsearch:never+gonna+give+you+up

Load a track.

**Example request:**
```bash
curl -X GET -H "Authorization: youshallnotpass" "http://localhost:2333/v4/loadtracks?identifier=ytsearch:never+gonna+give+you+up"
```

<details>
<summary><b>Raw API response (Expand)</b></summary>

```json
{
  "loadType": "search",
  "data": [
    {
      "encoded": "QAAA4wMARFJpY2sgQXN0bGV5IC0gTmV2ZXIgR29ubmEgR2l2ZSBZb3UgVXAgKE9mZmljaWFsIFZpZGVvKSAoNEsgUmVtYXN0ZXIpAAtSaWNrIEFzdGxleQAAAAAAA0PwAAtkUXc0dzlXZ1hjUQABACtodHRwczovL3d3dy55b3V0dWJlLmNvbS93YXRjaD92PWRRdzR3OVdnWGNRAQA2aHR0cHM6Ly9pLnl0aW1nLmNvbS92aV93ZWJwL2RRdzR3OVdnWGNRL3NkZGVmYXVsdC53ZWJwAAAHeW91dHViZQAAAAAAAAAA",
      "info": {
        "identifier": "dQw4w9WgXcQ",
        "isSeekable": true,
        "author": "Rick Astley",
        "length": 214000,
        "isStream": false,
        "position": 0,
        "title": "Rick Astley - Never Gonna Give You Up (Official Video) (4K Remaster)",
        "uri": "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "artworkUrl": "https://i.ytimg.com/vi_webp/dQw4w9WgXcQ/sddefault.webp",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAAwQMAJVJpY2sgQXN0bGV5IC0gTmV2ZXIgR29ubmEgR2l2ZSBZb3UgVXAADkFtYXppbmcgTHlyaWNzAAAAAAADQ/AACzdGd0RQMTdYUGxrAAEAK2h0dHBzOi8vd3d3LnlvdXR1YmUuY29tL3dhdGNoP3Y9N0Z3RFAxN1hQbGsBADBodHRwczovL2kueXRpbWcuY29tL3ZpLzdGd0RQMTdYUGxrL3NkZGVmYXVsdC5qcGcAAAd5b3V0dWJlAAAAAAAAAAA=",
      "info": {
        "identifier": "7FwDP17XPlk",
        "isSeekable": true,
        "author": "Amazing Lyrics",
        "length": 214000,
        "isStream": false,
        "position": 0,
        "title": "Rick Astley - Never Gonna Give You Up",
        "uri": "https://www.youtube.com/watch?v=7FwDP17XPlk",
        "artworkUrl": "https://i.ytimg.com/vi/7FwDP17XPlk/sddefault.jpg",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAA4gMAMUluc3VyQUFBbmNlICYgUmljayBBc3RsZXkgTmV2ZXIgR29ubmEgR2l2ZSBZb3UgVXAAI0NTQUEgSW5zdXJhbmNlIEdyb3VwLCBhIEFBQSBJbnN1cmVyAAAAAAAA/egAC0d0TDFodWluOUVFAAEAK2h0dHBzOi8vd3d3LnlvdXR1YmUuY29tL3dhdGNoP3Y9R3RMMWh1aW45RUUBADBodHRwczovL2kueXRpbWcuY29tL3ZpL0d0TDFodWluOUVFL3NkZGVmYXVsdC5qcGcAAAd5b3V0dWJlAAAAAAAAAAA=",
      "info": {
        "identifier": "GtL1huin9EE",
        "isSeekable": true,
        "author": "CSAA Insurance Group, a AAA Insurer",
        "length": 65000,
        "isStream": false,
        "position": 0,
        "title": "InsurAAAnce & Rick Astley Never Gonna Give You Up",
        "uri": "https://www.youtube.com/watch?v=GtL1huin9EE",
        "artworkUrl": "https://i.ytimg.com/vi/GtL1huin9EE/sddefault.jpg",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAA0QMAQE5ldmVyIEdvbm5hIEdpdmUgWW91IFVwIHwgUmljayBBc3RsZXkgUm9ja3MgTmV3IFllYXIncyBFdmUgLSBCQkMAA0JCQwAAAAAAA5IQAAtYR3hJRTFocjB3NAABACtodHRwczovL3d3dy55b3V0dWJlLmNvbS93YXRjaD92PVhHeElFMWhyMHc0AQAwaHR0cHM6Ly9pLnl0aW1nLmNvbS92aS9YR3hJRTFocjB3NC9zZGRlZmF1bHQuanBnAAAHeW91dHViZQAAAAAAAAAA",
      "info": {
        "identifier": "XGxIE1hr0w4",
        "isSeekable": true,
        "author": "BBC",
        "length": 234000,
        "isStream": false,
        "position": 0,
        "title": "Never Gonna Give You Up | Rick Astley Rocks New Year's Eve - BBC",
        "uri": "https://www.youtube.com/watch?v=XGxIE1hr0w4",
        "artworkUrl": "https://i.ytimg.com/vi/XGxIE1hr0w4/sddefault.jpg",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAAwQMAJEZhbWlseSBHdXkgLSBOZXZlciBHb25uYSBHaXZlIFlvdSBVcAAPQXJyaWYgSmFsYWx1ZGluAAAAAAABpeAAC0RzQzhqUVhSYlFFAAEAK2h0dHBzOi8vd3d3LnlvdXR1YmUuY29tL3dhdGNoP3Y9RHNDOGpRWFJiUUUBADBodHRwczovL2kueXRpbWcuY29tL3ZpL0RzQzhqUVhSYlFFL2hxZGVmYXVsdC5qcGcAAAd5b3V0dWJlAAAAAAAAAAA=",
      "info": {
        "identifier": "DsC8jQXRbQE",
        "isSeekable": true,
        "author": "Arrif Jalaludin",
        "length": 108000,
        "isStream": false,
        "position": 0,
        "title": "Family Guy - Never Gonna Give You Up",
        "uri": "https://www.youtube.com/watch?v=DsC8jQXRbQE",
        "artworkUrl": "https://i.ytimg.com/vi/DsC8jQXRbQE/hqdefault.jpg",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAAxgMAJ05ldmVyIEdvbm5hIEdpdmUgWW91IFVwICgyMDIyIFJlbWFzdGVyKQALUmljayBBc3RsZXkAAAAAAAND8AALM0JGVGlvNTI5NncAAQAraHR0cHM6Ly93d3cueW91dHViZS5jb20vd2F0Y2g/dj0zQkZUaW81Mjk2dwEANmh0dHBzOi8vaS55dGltZy5jb20vdmlfd2VicC8zQkZUaW81Mjk2dy9zZGRlZmF1bHQud2VicAAAB3lvdXR1YmUAAAAAAAAAAA==",
      "info": {
        "identifier": "3BFTio5296w",
        "isSeekable": true,
        "author": "Rick Astley",
        "length": 214000,
        "isStream": false,
        "position": 0,
        "title": "Never Gonna Give You Up (2022 Remaster)",
        "uri": "https://www.youtube.com/watch?v=3BFTio5296w",
        "artworkUrl": "https://i.ytimg.com/vi_webp/3BFTio5296w/sddefault.webp",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAA2QMAOuOAkOaXpeacrOiqnuWtl+W5leOAkVJpY2sgQXN0bGV5IC0gTmV2ZXIgR29ubmEgR2l2ZSBZb3UgVXAAC1JpY2sgQXN0bGV5AAAAAAADQ/AAC1JyRVN2U1JOcGVvAAEAK2h0dHBzOi8vd3d3LnlvdXR1YmUuY29tL3dhdGNoP3Y9UnJFU3ZTUk5wZW8BADZodHRwczovL2kueXRpbWcuY29tL3ZpX3dlYnAvUnJFU3ZTUk5wZW8vc2RkZWZhdWx0LndlYnAAAAd5b3V0dWJlAAAAAAAAAAA=",
      "info": {
        "identifier": "RrESvSRNpeo",
        "isSeekable": true,
        "author": "Rick Astley",
        "length": 214000,
        "isStream": false,
        "position": 0,
        "title": "【日本語字幕】Rick Astley - Never Gonna Give You Up",
        "uri": "https://www.youtube.com/watch?v=RrESvSRNpeo",
        "artworkUrl": "https://i.ytimg.com/vi_webp/RrESvSRNpeo/sddefault.webp",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAAywMALk5ldmVyIEdvbm5hIEdpdmUgWW91IFVwIC0gUmljayBBc3RsZXkgKEx5cmljcykAD0ludml0ZWQgS2luZ2RvbQAAAAAAA0PwAAtNT2NRYVI3X1dybwABACtodHRwczovL3d3dy55b3V0dWJlLmNvbS93YXRjaD92PU1PY1FhUjdfV3JvAQAwaHR0cHM6Ly9pLnl0aW1nLmNvbS92aS9NT2NRYVI3X1dyby9zZGRlZmF1bHQuanBnAAAHeW91dHViZQAAAAAAAAAA",
      "info": {
        "identifier": "MOcQaR7_Wro",
        "isSeekable": true,
        "author": "Invited Kingdom",
        "length": 214000,
        "isStream": false,
        "position": 0,
        "title": "Never Gonna Give You Up - Rick Astley (Lyrics)",
        "uri": "https://www.youtube.com/watch?v=MOcQaR7_Wro",
        "artworkUrl": "https://i.ytimg.com/vi/MOcQaR7_Wro/sddefault.jpg",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAAyQMANUJhcnJ5IFdoaXRlIC0gTmV2ZXIgTmV2ZXIgR29ubmEgR2l2ZSBZYSBVcCDigKIgVG9wUG9wAAZUb3BQb3AAAAAAAAOhsAALSzV6UDdlUWx0REUAAQAraHR0cHM6Ly93d3cueW91dHViZS5jb20vd2F0Y2g/dj1LNXpQN2VRbHRERQEAMGh0dHBzOi8vaS55dGltZy5jb20vdmkvSzV6UDdlUWx0REUvc2RkZWZhdWx0LmpwZwAAB3lvdXR1YmUAAAAAAAAAAA==",
      "info": {
        "identifier": "K5zP7eQltDE",
        "isSeekable": true,
        "author": "TopPop",
        "length": 238000,
        "isStream": false,
        "position": 0,
        "title": "Barry White - Never Never Gonna Give Ya Up • TopPop",
        "uri": "https://www.youtube.com/watch?v=K5zP7eQltDE",
        "artworkUrl": "https://i.ytimg.com/vi/K5zP7eQltDE/sddefault.jpg",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAAvAMAHU5ldmVyLCBOZXZlciBHb25uYSBHaXZlIFlhIFVwAAtCYXJyeSBXaGl0ZQAAAAAAB0dIAAtRcGJoU2xjZV9lawABACtodHRwczovL3d3dy55b3V0dWJlLmNvbS93YXRjaD92PVFwYmhTbGNlX2VrAQA2aHR0cHM6Ly9pLnl0aW1nLmNvbS92aV93ZWJwL1FwYmhTbGNlX2VrL3NkZGVmYXVsdC53ZWJwAAAHeW91dHViZQAAAAAAAAAA",
      "info": {
        "identifier": "QpbhSlce_ek",
        "isSeekable": true,
        "author": "Barry White",
        "length": 477000,
        "isStream": false,
        "position": 0,
        "title": "Never, Never Gonna Give Ya Up",
        "uri": "https://www.youtube.com/watch?v=QpbhSlce_ek",
        "artworkUrl": "https://i.ytimg.com/vi_webp/QpbhSlce_ek/sddefault.webp",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAA4wMARFJpY2sgQXN0bGV5ICAtIE5ldmVyIEdvbm5hIEdpdmUgWW91IFVwIChQaWFub2ZvcnRlKSAoT2ZmaWNpYWwgQXVkaW8pAAtSaWNrIEFzdGxleQAAAAAAAzRQAAtHSE1qRDBMcDVEWQABACtodHRwczovL3d3dy55b3V0dWJlLmNvbS93YXRjaD92PUdITWpEMExwNURZAQA2aHR0cHM6Ly9pLnl0aW1nLmNvbS92aV93ZWJwL0dITWpEMExwNURZL3NkZGVmYXVsdC53ZWJwAAAHeW91dHViZQAAAAAAAAAA",
      "info": {
        "identifier": "GHMjD0Lp5DY",
        "isSeekable": true,
        "author": "Rick Astley",
        "length": 210000,
        "isStream": false,
        "position": 0,
        "title": "Rick Astley  - Never Gonna Give You Up (Pianoforte) (Official Audio)",
        "uri": "https://www.youtube.com/watch?v=GHMjD0Lp5DY",
        "artworkUrl": "https://i.ytimg.com/vi_webp/GHMjD0Lp5DY/sddefault.webp",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAA+gMAWVLEq8SLYSDEkmFzdGzEk2FoIC0gTmV2ZXIgZ29ubmEgZ2l2ZSB5b3UgdXAgQ292ZXIgSW4gT2xkIEVuZ2xpc2guIEJhcmRjb3JlL01lZGlldmFsIHN0eWxlABN0aGVfbWlyYWNsZV9hbGlnbmVyAAAAAAACfLgAC2NFcmdNSlNncHYwAAEAK2h0dHBzOi8vd3d3LnlvdXR1YmUuY29tL3dhdGNoP3Y9Y0VyZ01KU2dwdjABADBodHRwczovL2kueXRpbWcuY29tL3ZpL2NFcmdNSlNncHYwL3NkZGVmYXVsdC5qcGcAAAd5b3V0dWJlAAAAAAAAAAA=",
      "info": {
        "identifier": "cErgMJSgpv0",
        "isSeekable": true,
        "author": "the_miracle_aligner",
        "length": 163000,
        "isStream": false,
        "position": 0,
        "title": "Rīċa Ēastlēah - Never gonna give you up Cover In Old English. Bardcore/Medieval style",
        "uri": "https://www.youtube.com/watch?v=cErgMJSgpv0",
        "artworkUrl": "https://i.ytimg.com/vi/cErgMJSgpv0/sddefault.jpg",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAA3gMAP1JpY2sgQXN0bGV5IC0gTmV2ZXIgR29ubmEgR2l2ZSBZb3UgVXAgKE9mZmljaWFsIEFuaW1hdGVkIFZpZGVvKQALUmljayBBc3RsZXkAAAAAAANACAALTExGaEthcW5Xd2sAAQAraHR0cHM6Ly93d3cueW91dHViZS5jb20vd2F0Y2g/dj1MTEZoS2Fxbld3awEANmh0dHBzOi8vaS55dGltZy5jb20vdmlfd2VicC9MTEZoS2Fxbld3ay9zZGRlZmF1bHQud2VicAAAB3lvdXR1YmUAAAAAAAAAAA==",
      "info": {
        "identifier": "LLFhKaqnWwk",
        "isSeekable": true,
        "author": "Rick Astley",
        "length": 213000,
        "isStream": false,
        "position": 0,
        "title": "Rick Astley - Never Gonna Give You Up (Official Animated Video)",
        "uri": "https://www.youtube.com/watch?v=LLFhKaqnWwk",
        "artworkUrl": "https://i.ytimg.com/vi_webp/LLFhKaqnWwk/sddefault.webp",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAA1QMALlJpY2sgQXN0bGV5IC0gTmV2ZXIgR29ubmEgR2l2ZSBZb3UgVXAgKEx5cmljcykAE1lvdW5nIFBpbGdyaW0gTXVzaWMAAAAAAAN6oAALNlBMYXRQTW94R3cAAQAraHR0cHM6Ly93d3cueW91dHViZS5jb20vd2F0Y2g/dj02UExhdFBNb3hHdwEANmh0dHBzOi8vaS55dGltZy5jb20vdmlfd2VicC82UExhdFBNb3hHdy9zZGRlZmF1bHQud2VicAAAB3lvdXR1YmUAAAAAAAAAAA==",
      "info": {
        "identifier": "6PLatPMoxGw",
        "isSeekable": true,
        "author": "Young Pilgrim Music",
        "length": 228000,
        "isStream": false,
        "position": 0,
        "title": "Rick Astley - Never Gonna Give You Up (Lyrics)",
        "uri": "https://www.youtube.com/watch?v=6PLatPMoxGw",
        "artworkUrl": "https://i.ytimg.com/vi_webp/6PLatPMoxGw/sddefault.webp",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAA1gMAOExpc2EgU3RhbnNmaWVsZCAtIE5ldmVyLCBOZXZlciBHb25uYSBHaXZlIFlvdSBVcCAoVmlkZW8pABBMaXNhU3RhbnNmaWVsZHR2AAAAAAAEFuAAC3B6WXBIWEN1bUlJAAEAK2h0dHBzOi8vd3d3LnlvdXR1YmUuY29tL3dhdGNoP3Y9cHpZcEhYQ3VtSUkBADBodHRwczovL2kueXRpbWcuY29tL3ZpL3B6WXBIWEN1bUlJL3NkZGVmYXVsdC5qcGcAAAd5b3V0dWJlAAAAAAAAAAA=",
      "info": {
        "identifier": "pzYpHXCumII",
        "isSeekable": true,
        "author": "LisaStansfieldtv",
        "length": 268000,
        "isStream": false,
        "position": 0,
        "title": "Lisa Stansfield - Never, Never Gonna Give You Up (Video)",
        "uri": "https://www.youtube.com/watch?v=pzYpHXCumII",
        "artworkUrl": "https://i.ytimg.com/vi/pzYpHXCumII/sddefault.jpg",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAA6QMAT1JpY2sgQXN0bGV5IC0gTmV2ZXIgR29ubmEgR2l2ZSBZb3UgVXAgW1JlbWFzdGVyZWQgSW4gNEtdIChPZmZpY2lhbCBNdXNpYyBWaWRlbykADEVuam95IGl08J+kjQAAAAAAAzwgAAtMUTR3OXhpSGtyWQABACtodHRwczovL3d3dy55b3V0dWJlLmNvbS93YXRjaD92PUxRNHc5eGlIa3JZAQAwaHR0cHM6Ly9pLnl0aW1nLmNvbS92aS9MUTR3OXhpSGtyWS9zZGRlZmF1bHQuanBnAAAHeW91dHViZQAAAAAAAAAA",
      "info": {
        "identifier": "LQ4w9xiHkrY",
        "isSeekable": true,
        "author": "Enjoy it🤍",
        "length": 212000,
        "isStream": false,
        "position": 0,
        "title": "Rick Astley - Never Gonna Give You Up [Remastered In 4K] (Official Music Video)",
        "uri": "https://www.youtube.com/watch?v=LQ4w9xiHkrY",
        "artworkUrl": "https://i.ytimg.com/vi/LQ4w9xiHkrY/sddefault.jpg",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAAywMALk5ldmVyIEdvbm5hIEdpdmUgWW91IFVwIChMeXJpY3MpIC0gUmljayBBc3RsZXkAD0dpb3Zhbm5hIExvemFubwAAAAAAA0AIAAtTYllYa09Bb1pwSQABACtodHRwczovL3d3dy55b3V0dWJlLmNvbS93YXRjaD92PVNiWVhrT0FvWnBJAQAwaHR0cHM6Ly9pLnl0aW1nLmNvbS92aS9TYllYa09Bb1pwSS9zZGRlZmF1bHQuanBnAAAHeW91dHViZQAAAAAAAAAA",
      "info": {
        "identifier": "SbYXkOAoZpI",
        "isSeekable": true,
        "author": "Giovanna Lozano",
        "length": 213000,
        "isStream": false,
        "position": 0,
        "title": "Never Gonna Give You Up (Lyrics) - Rick Astley",
        "uri": "https://www.youtube.com/watch?v=SbYXkOAoZpI",
        "artworkUrl": "https://i.ytimg.com/vi/SbYXkOAoZpI/sddefault.jpg",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAA4gMAO1JpY2sgQXN0bGV5IC0gTkVWRVIgR09OTkEgR0lWRSBZT1UgVVAgKFN1bmcgYnkgMTY5IE1vdmllcyEpABNUaGUgVW51c3VhbCBTdXNwZWN0AAAAAAACm/gAC2VweVJVcDBCaHJBAAEAK2h0dHBzOi8vd3d3LnlvdXR1YmUuY29tL3dhdGNoP3Y9ZXB5UlVwMEJockEBADZodHRwczovL2kueXRpbWcuY29tL3ZpX3dlYnAvZXB5UlVwMEJockEvc2RkZWZhdWx0LndlYnAAAAd5b3V0dWJlAAAAAAAAAAA=",
      "info": {
        "identifier": "epyRUp0BhrA",
        "isSeekable": true,
        "author": "The Unusual Suspect",
        "length": 171000,
        "isStream": false,
        "position": 0,
        "title": "Rick Astley - NEVER GONNA GIVE YOU UP (Sung by 169 Movies!)",
        "uri": "https://www.youtube.com/watch?v=epyRUp0BhrA",
        "artworkUrl": "https://i.ytimg.com/vi_webp/epyRUp0BhrA/sddefault.webp",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    },
    {
      "encoded": "QAAA3wMAQFJpY2sgQXN0bGV5IC0gTmV2ZXIgR29ubmEgR2l2ZSBZb3UgVXAgKFBpYW5vZm9ydGUpIChQZXJmb3JtYW5jZSkAC1JpY2sgQXN0bGV5AAAAAAADX0gAC3JUZ2E0MXIzYTRzAAEAK2h0dHBzOi8vd3d3LnlvdXR1YmUuY29tL3dhdGNoP3Y9clRnYTQxcjNhNHMBADZodHRwczovL2kueXRpbWcuY29tL3ZpX3dlYnAvclRnYTQxcjNhNHMvc2RkZWZhdWx0LndlYnAAAAd5b3V0dWJlAAAAAAAAAAA=",
      "info": {
        "identifier": "rTga41r3a4s",
        "isSeekable": true,
        "author": "Rick Astley",
        "length": 221000,
        "isStream": false,
        "position": 0,
        "title": "Rick Astley - Never Gonna Give You Up (Pianoforte) (Performance)",
        "uri": "https://www.youtube.com/watch?v=rTga41r3a4s",
        "artworkUrl": "https://i.ytimg.com/vi_webp/rTga41r3a4s/sddefault.webp",
        "isrc": null,
        "sourceName": "youtube"
      },
      "pluginInfo": {},
      "userData": {}
    }
  ]
}
```
</details>

## GET /v4/sessions/xyz

Get session information (returns 404/Error if invalid).

**Example request:**
```bash
curl -X GET -H "Authorization: youshallnotpass" "http://localhost:2333/v4/sessions/xyz"
```

<details>
<summary><b>Raw API response (Expand)</b></summary>

```json
{
  "timestamp": 1773058695473,
  "status": 404,
  "error": "Not Found",
  "message": "Session not found: xyz",
  "path": "/v4/sessions/xyz"
}
```
</details>

## PATCH /v4/sessions/xyz

Update session properties.

**Example request:**
```bash
curl -X PATCH -H "Authorization: youshallnotpass" -H "Content-Type: application/json" -d '{"resuming": true, "timeout": 60}' "http://localhost:2333/v4/sessions/xyz"
```

<details>
<summary><b>Raw API response (Expand)</b></summary>

```json
{
  "timestamp": 1773058695481,
  "status": 404,
  "error": "Not Found",
  "message": "Session not found",
  "path": "/v4/sessions/xyz"
}
```
</details>

