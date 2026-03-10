# Configuration

Rustalink relies on a `config.toml` file for predictable, readable configuration.

## Example

```toml
[server]
port = 2333
address = "0.0.0.0"
password = "youshallnotpass"

[audio]
item_timeout_ms = 3000

[plugins]
# Plugin configurations go here
```
