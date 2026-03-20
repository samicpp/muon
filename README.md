# muon
A rust webserver built for ease of use, and speed

## Crate features
| feature      | requires | desciption                       |
|--------------|----------|----------------------------------|
| unix-sockets | -        | allows unix sockets in ffi types |
| ring         | -        | uses ring provider               |
| aws-lc-rs    | -        | uses aws provider                |
| simple       | -        | enables the simple handler       |
| samicpp      | -        | enables the samicpp handler      |


## Usage
muon tries to read a file named `settings.toml` from the directory its executable lies. <br/>
Alternatively a different file can be specified with cli argument `--settings ./path/to/file.toml` <br/>


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