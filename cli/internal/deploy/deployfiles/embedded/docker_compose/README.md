# Welcome to Lumen

To set up Lumen there are several options, Lumen supports the following for deployment:
1. Quick guided install via the install.sh script
2. Pulling the repo and running `docker compose up -d` from the deployment/docker_compose directory
  - Note, it is recommended to copy over the env.template file to .env and edit the necessary values
3. For large scale deployments leveraging Kubernetes, there are two options, Helm or Terraform.

This README focuses on the easiest guided deployment which is via install.sh.

**For more detailed guides, please refer to the documentation: https://docs.lumen.app/deployment/overview**

## install.sh script

```
curl -fsSL https://raw.githubusercontent.com/lumen-dot-app/lumen/main/deployment/docker_compose/install.sh > install.sh && chmod +x install.sh && ./install.sh
```

The script installs the Lumen CLI (`lumen-cli`) and hands over to `lumen-cli deploy install`, which is
where the guided installation lives. Any flags you pass are forwarded to it. If you already have the
CLI (`pip install lumen-cli`), skip the script and run `lumen-cli deploy install` directly.

This provides a guided installation of Lumen via Docker Compose. It will deploy the latest version of Lumen
and set up the volumes to ensure data is persisted across deployments or upgrades.

The deployment files are stored in `~/.config/lumen` (an existing `lumen_data` directory from an older
install is detected and kept in place; `--dir` targets another location). Note that no application
critical data is stored in that directory so even if you delete it, the data needed to restore the app
will not be destroyed.

The data about chats, users, etc. are instead stored as named Docker Volumes. This is managed by Docker
and where it is stored will depend on your Docker setup. You can always delete these as well by running
`lumen-cli deploy uninstall`.

To shut down the deployment without deleting, use `lumen-cli deploy stop`.

### Managing the deployment

Beyond installing, the CLI covers the rest of the lifecycle:

| Command | What it does |
| --- | --- |
| `lumen-cli deploy status` | Installed version, containers, and health (`--json` for scripts) |
| `lumen-cli deploy logs [service...]` | Logs of the deployment's containers |
| `lumen-cli deploy stop` | Stop the containers, keep the data |
| `lumen-cli deploy upgrade [--tag vX.Y.Z]` | Upgrade in place (see below) |
| `lumen-cli deploy uninstall` | Remove the containers, volumes, and deployment directory |

### Upgrading the deployment
Lumen maintains backwards compatibility across all minor versions following SemVer, so upgrading is
`lumen-cli deploy upgrade` (add `--tag vX.Y.Z` to pin a version). It rewrites only IMAGE_TAG, preserves
your .env edits, and backs up hand-edited files.

If you are more comfortable running docker compose commands, you can also run commands directly from
the directory with the docker-compose.yml file. First bring the containers down (`docker compose down`),
verify the version you want in the environment file (see below), (if using `latest` tag, be sure to run
`docker compose pull`) and run `docker compose up` to restart the services on the latest version

### Customizing the compose files
The CLI owns the compose files it writes. Each `lumen-cli deploy upgrade` refreshes them to match the
new version. If you edit one, the CLI asks before it replaces the file, and it keeps a backup — but
your edits are not applied again.

To customize the deployment, put your changes in a file next to `docker-compose.yml`, named
`compose.override.yml`, `compose.override.yaml`, `docker-compose.override.yml`, or
`docker-compose.override.yaml`. These are the same names `docker compose` finds on its own; if more than
one is present, the CLI picks the same one Compose would, in that order. The CLI applies that file last,
after its own files, so your changes win. The CLI does not manage the override: it never writes,
replaces, or backs it up, and an upgrade does not touch it. `install`, `upgrade`, `stop`, `logs`, and
`uninstall` all apply it.

For example, to put Lumen behind your own reverse proxy, stop nginx from publishing a host port and
attach it to your proxy's network:

```yaml
# deployment/docker-compose.override.yml
services:
  nginx:
    # !reset replaces the list. Without it, Compose adds to the list.
    ports: !reset []
    networks: [default, proxy]

networks:
  proxy:
    external: true
```

Use `docker compose -f docker-compose.yml -f docker-compose.override.yml config` in the deployment
directory to see the merged result. Add each file the CLI applies (`docker-compose.lumen-lite.yml`,
`docker-compose.prod.yml`, and so on) in the same order the CLI does.

### Environment variables
The Docker Compose files try to look for a .env file in the same directory. The installer sets it up
from a file called env.template. Feel free to edit the .env file to customize your deployment. The most
important / common changed values are located near the top of the file. Later `lumen-cli deploy` runs
keep your edits.

IMAGE_TAG is the version of Lumen to run. It is recommended to leave it as latest to get all updates with each redeployment.

Every image publishes a `-dev` twin for each of its tags (e.g. `latest-dev`, `v1.2.3-dev`), so a single
`IMAGE_TAG=latest-dev` selects the dev variant of the whole deployment. Today only the backend image actually differs:
its `-dev` twin adds interactive debugging tools (vim, nano, curl, ps, psql) that the default image leaves out to stay
minimal. The web-server, model-server, and sandbox `-dev` tags are identical to their plain counterparts and exist so
that one version string covers every image.
