<h1 align="center">
  <br>
  <img src="https://raw.githubusercontent.com/YusongWang/mining_proxy/9ec34e9d780866ab8792df09a9d6ec0b0f01b013/images/logo.png" width="350"/>
</h1>

<h2 align="center">Fully open source - no built-in developer wallet</h2>
<h4 align="center">Rust language-based ETH/ETC/CFX proxy pumping software based on tokio ecology</h4>

<p align="center">
  <a>
    <img src="https://img.shields.io/badge/Release-v0.2.2-orgin.svg" alt="travis">
  </a>
  <a>
    <img src="https://img.shields.io/badge/Last_Update-2022_02_08-orgin.svg" alt="travis">
  </a>
  <a>
    <img src="https://img.shields.io/badge/Language-Rust-green.svg" alt="travis">
  </a>
  <a>
    <img src="https://img.shields.io/badge/License-Apache-green.svg" alt="travis">
  </a>
</p>
<p align="center">See Release for the latest version <a href="https://github.com/YusongWang/mining_proxy/releases">Github Release</a></p>
<p align="center">Historical releases: https://github.com/dothinkdone/mining_proxy/releases</p>
<p align="center">
Coffee: Eth+BSC+HECO+Matic: 0x3602b50d3086edefcd9318bcceb6389004fb14ee
</p>

<p align="center">
  <a href="https://t.me/+ZkUDlH2Fecc3MGM1">Telegram group</a> •
  <a href="https://jq.qq.com/?_wv=1027&k=AWfknDiw">QQ group</a>
</p>

![Screenshot](https://raw.githubusercontent.com/YusongWang/mining_proxy/master/doc/images/web.jpg)

## :sparkles: features

- :cloud: Support ETH ETC CFX forwarding
- :zap: High performance and low CPU usage.
- 💻 You can customize the draw ratio
- 📚 You can customize the pumping algorithm.
- 💾 Safe and stable: Supports 3 different protocols including TCP SSL and encryption methods (lightweight algorithms, non-SSR garbage)
- :outbox_tray: A machine only needs to open a web interface. Configurable multi-pool forwarding (no upper limit)
- :rocket: Out of the box: All-In-One packaging, one-click build and run, one-click configuration
- :family_woman_girl_boy: supports Liunx Windows

## :hammer_and_wrench: deploy

- self-compile
Compilation problems are basically the source code of the web without clone
See here: https://github.com/YusongWang/mining_proxy/issues/26

Create a .env file in the software running directory
````env
MINING_PROXY_WEB_PORT=8020
MINING_PROXY_WEB_PASSWORD=123456789
JWT_SECRET=test
````
The first line is the port of the web page
The second line is the password for web management
The third line is the encryption key for the login password. It is recommended to use a random string of at least 32 bits


## other instructions
<a href="https://github.com/YusongWang/mining_proxy_web">Web interface address</a><br>