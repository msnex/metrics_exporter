# docker

Grafana 预装插件镜像的统一镜像名为 `localhost/grafana-custom:13.2`，docker 与 podman
构建产物一致；`docker/docker-compose.yml` 与 `docker/podman/*.service` 均引用该名，
不直接使用上游镜像。

## 构建镜像（需联网，先于运行）

```bash
# docker
docker build --build-arg "GRAFANA_VERSION=13.2" \
  --build-arg "GF_INSTALL_PLUGINS=victoriametrics-metrics-datasource" \
  -t localhost/grafana-custom:13.2 .
```

```bash
# podman
podman build --build-arg-file argfile.conf -t localhost/grafana-custom:13.2 .
```

离线机器：在联网机构建后 `docker save` / `podman save` 导出 tar，目标机
`docker load` / `podman load` 导入（统一 tag 原样保留，无需改任何引用）。

## 运行（docker compose / podman）

compose 仅引用本地镜像，须先构建或导入，之后：

```bash
# docker
docker compose -f docker/docker-compose.yml up -d
# podman（按目标机安装的工具二选一）
podman-compose up -d
# 或
podman compose up -d
```

`docker/provisioning/` 是**运行时绑定挂载**（`/etc/grafana/provisioning`），属于经常
修改的数据、不打包进镜像：dashboard 改动约 30s 内热加载，datasource 改动重启
grafana 生效，均无需重建镜像。

## systemd 部署（podman 单元，服务端）

```bash
sudo cp -r docker/provisioning /etc/grafana/provisioning
sudo cp docker/offline-resolv.conf docker/offline-hosts /etc/grafana/
sudo systemctl daemon-reload
sudo systemctl enable --now victoriametrics.service grafana.service
```

单元以宿主机 `/etc/grafana/provisioning` 只读挂载进容器，改文件后按上述规则生效。
