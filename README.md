<div align="center">
  <a href="https://discord.shaybox.com">
    <img alt="Discord" src="https://img.shields.io/discord/824865729445888041?color=404eed&label=Discord&logo=Discord&logoColor=FFFFFF">
  </a>
  <a href="https://github.com/shaybox/vrc-yt/releases/latest">
    <img alt="Downloads" src="https://img.shields.io/github/downloads/shaybox/vrc-yt/total?color=3fb950&label=Downloads&logo=github&logoColor=FFFFFF">
  </a>
</div>

# VRC-YT
Playing [YouTube] videos cross-platform in [VRChat]

## Notice:
VRChat added `yt-dlp` support to Android/Quest, making this proxy useless.  
YouTube is cracking down on IPs and Accounts using `yt-dlp`, making this proxy useless.

## Proxy
The [proxy](/proxy) package contains the source code for my [public proxy](https://shay.loan)  
You can also self-host your own instance using a [release](https://github.com/shaybox/vrc-yt/releases/latest) build  
This proxy was inspired by [vroxy](https://github.com/techanon/vroxy) which has similar functionality  


### VRChat Users
If you're not a [World Creator](#vrchat-world-creators)  
You can use my proxy in any world with a compatible video player

⬇️The original YouTube video URL  
`https://www.youtube.com/watch?v=dQw4w9WgXcQ`  
⬇️You don't have to type the rest of the URL  
`https://shay.loan/dQw4w9WgXcQ`


### VRChat World Creators
You must use a video player that supports Quest  
Such as AVPro or ProTV which uses AVPro

When you enter the YouTube URL into Unity  
Prefix the link with the proxy link  

⬇️The original YouTube video URL  
`https://www.youtube.com/watch?v=dQw4w9WgXcQ`  
⬇️Add the Proxy URL Prefix  
`https://shay.loan/https://www.youtube.com/watch?v=dQw4w9WgXcQ`  


### Self-Hosting:
You can self-host your own instance using the [latest release](https://github.com/ShayBox/VRC-YT/releases/latest)

The default binding address and port are intended for use behind a reverse proxy,  
Should you not want to use a reverse proxy and accept connections from remote hosts,  
You can change the address and port using the `ROCKET_ADDRESS` and `ROCKER_PORT` environment variables.

The default binding address `127.0.0.1` only accepts local hosts, using `0.0.0.0` accepts all hosts.  
The default binding port is `8000`, ports below 1024 usually require root privilege on Linux.


[YouTube]: https://youtube.com
[VRChat]:  https://vrchat.com
