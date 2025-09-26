# Store-Front Application
## Features
- store profile management, such as staff, business hour, and available products for sale, while enforcing role-based access control (RBAC) and quota limits
- store staff can maintain pricing plan for each product with multi-currency support

## High-Level Architecture

```mermaid
flowchart TD
    %% Clients subgraph with human icons
    subgraph Clients
      OIA(["👤 Other Internal Applications"])
      PU(["👤 Public Users"])
      ASS(["👤 Authorized Store Staff"])
      PS(["👤 Platform Staff"])
    end

    %% API Endpoints subgraph with only two nodes: Web and RPC
    subgraph API_Endpoints
      RPC[RPC]
      WEB[Web]
      WEBAUTH[Web authorised]
    end

    %% Service Layer subgraph with separated Saleable Item services
    subgraph Service_Layer
      BPROF[Store Basic Profile]
      PP[Product Price]
      SSU[Store Staff]
      BHU[Business Hour]
    end

    %% Data Store Layer subgraph
    subgraph Data_Store_Layer
      MARIA[MariaDB]
    end

    %% Connections from Clients to API Endpoints
    PU --> WEB
    ASS --> WEBAUTH
    PS --> WEBAUTH
    OIA --> RPC

    %% Connections from API Endpoints to Service Layer
    WEB --> BPROF
    WEBAUTH --> BPROF
    WEBAUTH --> PP
    WEBAUTH --> SSU
    WEBAUTH --> BHU
    RPC --> BPROF

    %% Connections from Service Layer to Data Store Layer
    BPROF --> MARIA
    PP --> MARIA
    SSU --> MARIA
    BHU --> MARIA
```

Note :
- currently, payment application may send request to this RPC endpoint through AMQP protocol

## Pre-requisite
| software | version | installation/setup guide |
|-----|-----|-----|
|Python | 3.13.7 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/build_python_from_source.md) |
|MariaDB| 11.8.2 | [see here](https://github.com/metalalive/EnvToolSetupJunkBox/blob/master/mariaDB/) |
|pipenv | 2025.0.4 | [see here](https://pip.pypa.io/en/stable/) |
|pip| 25.1 | [see here](https://pip.pypa.io/en/stable/) |

## Build
For full build / test instructions please refer to [this github action workflow script](../../.github/workflows/storefront-ci.yaml)
### For this application
First time to build / install modules for this application in per-project virtual environment:
```bash
PIPENV_VENV_IN_PROJECT=1 pipenv install --dev
```
Alternatively, you can switch to application base folder and explcitly specify virtual environment
```bash
PIPENV_VENV_IN_PROJECT=1 pipenv run python -m venv
```

- A virtual environment folder `.venv` will be created under the application folder `./store`
- Note [`Pipfile`](./Pipfile) already references path to [common python modules](../common/python), that makes `pipenv` installation automatically link to the common modules, no need to build the common python module explicitly.

Update the virtual environment after you are done editing `Pipfile` and `pyproject.toml` with the command :
```shell
pipenv update --dev
```

### Base Image for Application Environment
```bash
cd /path/to/project-home/services
docker image rm  storefront-backend-base:latest
docker build --tag=storefront-backend-base:latest --file=store/infra/Dockerfile  .
```

After custom image `storefront-backend-base:latest` is built successfully, use it for one of following tasks
- run application in development ensironment
- run all test cases

### Database Migration
Generate migration script template for ORM code update :
```bash
docker compose --file ./infra/docker-compose-db-generic.yml \
    --file ./infra/docker-compose-migration-codegen-generic.yml \
    --file ./infra/docker-compose-db-test.yml \
    --file ./infra/docker-compose-migration-codegen-test.yml \
    --env-file ./infra/interpolation-test.env  up --detach
```

The docker command above covers following basic alembic commands with all necessary variables / config files. 
```shell 
alembic --config alembic_app.ini  revision --autogenerate --rev-id <VERSION_NUMBER> \
   --depends-on  <PREVIOUS_VERSION_NUMBER>  -m "whatever_message"

// update
alembic --config alembic_app.ini upgrade  <VERSION_NUMBER>

// rollback
alembic --config alembic_app.ini downgrade  <VERSION_NUMBER>

// check all created revisions (might not sync yet to target database)
alembic --config alembic_app.ini history
```

Note
- after the docker command, check new migration script file under path `migrations/app/versions`
- the migration commands are the same as described in [alembic documentation](https://alembic.sqlalchemy.org/en/latest/tutorial.html)
- Alembic's auto-generated migration script should be reusable for all runtime environments in most case , no need to generate them for different environments
- `<VERSION_NUMBER>` can be the number which matches migration module under `migrations/app/versions` , for downgrade, `base` means rollback to state before any table is created in the database.


## Run
### API server and RPC consumer in Development Environment
```bash
docker compose \
    --file ./infra/docker-compose-db-generic.yml --file ./infra/docker-compose-srv-generic.yml \
    --file ./infra/docker-compose-db-dev.yml --file ./infra/docker-compose-srv-dev.yml \
    --env-file ./infra/interpolation-dev.env --profile serverstart up --detach
```

To run smoke test after dev server is launched, append extra `--file ./infra/docker-compose-smoketest4dev.yml`  to the end of `--file` option sequence.

## Test
### Integration Test
```bash
docker compose \
    --file ./infra/docker-compose-db-generic.yml --file ./infra/docker-compose-srv-generic.yml \
    --file ./infra/docker-compose-db-test.yml --file ./infra/docker-compose-srv-test.yml \
    --env-file ./infra/interpolation-test.env  --profile cleandbschema  up --detach
```

## Development
### Code Formatter
```bash
pipenv run black --line-length=100 ./src/ ./tests/  ./settings/ ./migrations
```

### Linting Check
```bash
pipenv run ruff check  ./src/ ./tests/  ./settings/ ./migrations
```
