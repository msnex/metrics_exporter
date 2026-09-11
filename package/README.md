# package

按**编译平台**分目录存放构建文件与产物，每个平台目录自包含（Dockerfile + 二进制）：

```
package/
├── README.md
└── rocky8/               # 编译平台：rockylinux/rockylinux:8 容器
    ├── Dockerfile
    └── metrics_exporter    # 产物（不入库）
```

| 平台目录  | 目标环境      | 产物                              |
| --------- | ------------- | --------------------------------- |
| `rocky8/` | Rocky Linux 8 | `package/rocky8/metrics_exporter` |

docker 与 podman 通用；构建镜像 tag 带平台后缀，与目录一一对应：
`localhost/metrics_exporter-builder:0.1.0-<平台名>`。

## 构建 Rocky 8

在仓库根目录执行：

```bash
# docker
docker build -f package/rocky8/Dockerfile \
  -t localhost/metrics_exporter-builder:0.1.0-rocky8 .
# podman（命令相同，引擎换成 podman）
podman build -f package/rocky8/Dockerfile \
  -t localhost/metrics_exporter-builder:0.1.0-rocky8 .
```

工具链默认取构建当时的 latest stable；需要完全复现时固定版本：

```bash
docker build --build-arg RUST_TOOLCHAIN=1.98.0 -f package/rocky8/Dockerfile \
  -t localhost/metrics_exporter-builder:0.1.0-rocky8 .
```

依赖版本由 Cargo.lock 锁定（`--locked`）。

## 拷出二进制

```bash
docker run --rm -v "$PWD/package/rocky8:/out" \
  localhost/metrics_exporter-builder:0.1.0-rocky8 \
  cp /src/target/release/metrics_exporter /out/
# podman：命令相同，引擎换成 podman
```

产物：`package/rocky8/metrics_exporter`（不入库）。

## 校验

```bash
file package/rocky8/metrics_exporter
sha256sum package/rocky8/metrics_exporter
# glibc 2.28 环境冒烟测试（等价于 rocky8 服务器运行能力）
docker run --rm -v "$PWD/package/rocky8:/pkg:ro" \
  rockylinux/rockylinux:8 /pkg/metrics_exporter --help
```

## 手动编译
```bash
docker run -ti --rm -v "$(pwd):/$(pwd)" --workdir "$(pwd)" localhost/metrics_exporter-builder:0.1.0-rocky8
cargo build --release --locked
exit
```

## 部署（rocky8 服务器，原生运行）

```bash
scp package/rocky8/metrics_exporter user@server:/usr/local/bin/
scp metrics_exporter.toml user@server:/etc/metrics_exporter/
# 服务器上（与 VictoriaMetrics host 网络 127.0.0.1:8428 配合）
/usr/local/bin/metrics_exporter -c /etc/metrics_exporter/metrics_exporter.toml
# systemd 单元安装见 ../docker/README.md
```

## 新增编译平台

1. 新建 `package/<平台名>/`（如 `rocky9.4/`；同平台多架构用 `rocky8-aarch64/`
   并在构建时加 `--platform`）；
2. 放入该平台 Dockerfile（可复制 `rocky8/` 并改 base image）；
3. 构建 tag 追加同一平台后缀（`localhost/metrics_exporter-builder:0.1.0-<平台名>`），
   产物拷入该平台目录；
4. 在本文档表格中登记。
