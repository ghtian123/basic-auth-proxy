
## BASIC-AUTH-PROXY

1. 解决k8s dashboard 前端页面 basic-auth 
2. 解决k8s dashboard 前端页面需要 https 输入token

```
Usage: basic-auth-proxy [OPTIONS]

Options:
  -l, --listen_addr <listen_addr>...  which addr to listen [default: 0.0.0.0:3000]
  -p, --proxy_addr <proxy_addr>...    which addr to proxy [default: https://www.baidu.com]
  -c, --cert_path <cert_path>...      cert path [default: ./basic-auth-proxy]
  -u, --user_passwd <user_passwd>...  user_passwd to auth,eg: test:test [default: test:test]
  -h, --help                          Print help information
```