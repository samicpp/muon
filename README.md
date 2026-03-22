# muon
A rust webserver built for ease of use, and speed

## Crate features
| feature      | requires | desciption                       |
|--------------|----------|----------------------------------|
| unix-sockets | -        | enables unix sockets             |
| ring         | -        | uses ring provider               |
| aws-lc-rs    | -        | uses aws provider                |
| simple       | -        | enables the simple handler       |
| samicpp      | -        | enables the samicpp handler      |


## Usage
muon tries to read a file named `settings.toml` from the directory its executable lies. <br/>
Alternatively a different file can be specified with cli argument `--settings ./path/to/file.toml` <br/>


## Handlers
| handler | description                     |
|---------|---------------------------------|
| simple  | a simple static content handler |
| samicpp | a handler with all of my needs  |


If you need a specific handler open an issue and i can make one named after you, <br/>
or fork the repo and open a pull request. <br/>


## TODO::Features
- [x] support TLS
- [x] TLS certificate selection
- [x] add loglevels based on individual logs
- [x] add simple handler
- [x] support multiple different addresses in same field
- [x] allow customizing tokio runtime
- [x] support cli arguments
- [x] support HTTP/2 and h2c
- [ ] support HTTP/1.1 pipelining
- [ ] advanced socket options with socket2
- [ ] colorize log output
- [ ] allow exporting/sending server connection data
- [ ] embed Deno engine to execute javascript
- [ ] enable loading FFI modules
- [ ] create runtime stdin console
- [ ] allow reloading settings file


## Examples
settings.toml
```toml
[network]
address = [
    "http://0.0.0.0:8001",
    "https://0.0.0.0:8002",
]
default_key = "./tls/localhost-key.pem"
default_cert = "./tls/localhost.pem"
alpn = [ "h2", "http/1.1" ]

[[network.sni]]
domain = "one.localhost"
key = "./tls/one.localhost-key.pem"
cert = "./tls/one.localhost.pem"

[[network.sni]]
domain = "two.localhost"
key = "./tls/two.localhost-key.pem"
cert = "./tls/two.localhost.pem"

[[network.binding]]
address = "httpx://0.0.0.0:2233"
backlog = 1


[environment]
cwd = "/var/www"


[content]
handler = "simple"
max_file_size = 16777216 # 16mb
serve_dir = "public"


[logging]

```